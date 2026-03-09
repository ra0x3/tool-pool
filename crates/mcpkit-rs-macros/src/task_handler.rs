use darling::{FromMeta, ast::NestedMeta};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Expr, ImplItem, ItemImpl};

#[derive(FromMeta)]
#[darling(default)]
struct TaskHandlerAttribute {
    processor: Expr,
}

impl Default for TaskHandlerAttribute {
    fn default() -> Self {
        Self {
            processor: syn::parse2(quote! { self.processor }).expect("default processor expr"),
        }
    }
}

pub fn task_handler(attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let attr_args = NestedMeta::parse_meta_list(attr)?;
    let TaskHandlerAttribute { processor } = TaskHandlerAttribute::from_list(&attr_args)?;
    let mut item_impl = syn::parse2::<ItemImpl>(input.clone())?;

    let has_method = |name: &str, item_impl: &ItemImpl| -> bool {
        item_impl.items.iter().any(|item| match item {
            ImplItem::Fn(func) => func.sig.ident == name,
            _ => false,
        })
    };

    if !has_method("list_tasks", &item_impl) {
        let list_fn = quote! {
            async fn list_tasks(
                &self,
                _request: Option<mcpkit_rs::model::PaginatedRequestParam>,
                _: mcpkit_rs::service::RequestContext<mcpkit_rs::RoleServer>,
            ) -> Result<mcpkit_rs::model::ListTasksResult, McpError> {
                let running_ids = (#processor).lock().await.list_running();
                let total = running_ids.len() as u64;
                let tasks = running_ids
                    .into_iter()
                    .map(|task_id| {
                        let timestamp = mcpkit_rs::task_manager::current_timestamp();
                        mcpkit_rs::model::Task::new(
                            task_id,
                            mcpkit_rs::model::TaskStatus::Working,
                            timestamp.clone(),
                            timestamp,
                        )
                    })
                    .collect::<Vec<_>>();

                Ok(mcpkit_rs::model::ListTasksResult::new(tasks))
            }
        };
        item_impl.items.push(syn::parse2::<ImplItem>(list_fn)?);
    }

    if !has_method("enqueue_task", &item_impl) {
        let enqueue_fn = quote! {
            async fn enqueue_task(
                &self,
                request: mcpkit_rs::model::CallToolRequestParam,
                context: mcpkit_rs::service::RequestContext<mcpkit_rs::RoleServer>,
            ) -> Result<mcpkit_rs::model::CreateTaskResult, McpError> {
                use mcpkit_rs::task_manager::{
                    current_timestamp, OperationDescriptor, OperationMessage, OperationResultTransport,
                    ToolCallTaskResult,
                };
                let task_id = context.id.to_string();
                let operation_name = request.name.to_string();
                let future_request = request.clone();
                let future_context = context.clone();
                let server = self.clone();

                let descriptor = OperationDescriptor::new(task_id.clone(), operation_name)
                    .with_context(context)
                    .with_client_request(mcpkit_rs::model::ClientRequest::CallToolRequest(
                        mcpkit_rs::model::Request::new(request),
                    ));

                let task_result_id = task_id.clone();
                let future = Box::pin(async move {
                    let result = server.call_tool(future_request, future_context).await;
                    Ok(
                        Box::new(ToolCallTaskResult::new(task_result_id, result))
                            as Box<dyn OperationResultTransport>,
                    )
                });

                (#processor)
                    .lock()
                    .await
                    .submit_operation(OperationMessage::new(descriptor, future))
                    .map_err(|err| mcpkit_rs::ErrorData::internal_error(
                        format!("failed to enqueue task: {err}"),
                        None,
                    ))?;

                let timestamp = current_timestamp();
                let task = mcpkit_rs::model::Task::new(
                    task_id,
                    mcpkit_rs::model::TaskStatus::Working,
                    timestamp.clone(),
                    timestamp,
                ).with_status_message("Task accepted");

                Ok(mcpkit_rs::model::CreateTaskResult::new(task))
            }
        };
        item_impl.items.push(syn::parse2::<ImplItem>(enqueue_fn)?);
    }

    if !has_method("get_task_info", &item_impl) {
        let get_info_fn = quote! {
            async fn get_task_info(
                &self,
                request: mcpkit_rs::model::GetTaskInfoParam,
                _context: mcpkit_rs::service::RequestContext<mcpkit_rs::RoleServer>,
            ) -> Result<mcpkit_rs::model::GetTaskResult, McpError> {
                use mcpkit_rs::task_manager::current_timestamp;
                let task_id = request.task_id.clone();
                let mut processor = (#processor).lock().await;

                // Check completed results first
                let completed = processor.peek_completed().iter().rev().find(|r| r.descriptor.operation_id == task_id);
                if let Some(completed_result) = completed {
                    // Determine Finished vs Failed
                    let status = match &completed_result.result {
                        Ok(boxed) => {
                            if let Some(tool) = boxed.as_any().downcast_ref::<mcpkit_rs::task_manager::ToolCallTaskResult>() {
                                match &tool.result {
                                    Ok(_) => mcpkit_rs::model::TaskStatus::Completed,
                                    Err(_) => mcpkit_rs::model::TaskStatus::Failed,
                                }
                            } else {
                                mcpkit_rs::model::TaskStatus::Completed
                            }
                        }
                        Err(_) => mcpkit_rs::model::TaskStatus::Failed,
                    };
                    let timestamp = current_timestamp();
                    let mut task = mcpkit_rs::model::Task::new(
                        task_id,
                        status,
                        timestamp.clone(),
                        timestamp,
                    );
                    if let Some(ttl) = completed_result.descriptor.ttl {
                        task = task.with_ttl(ttl);
                    }
                    return Ok(mcpkit_rs::model::GetTaskResult { meta: None, task });
                }

                // If not completed, check running
                let running = processor.list_running();
                if running.into_iter().any(|id| id == task_id) {
                    let timestamp = current_timestamp();
                    let task = mcpkit_rs::model::Task::new(
                        task_id,
                        mcpkit_rs::model::TaskStatus::Working,
                        timestamp.clone(),
                        timestamp,
                    );
                    return Ok(mcpkit_rs::model::GetTaskResult { meta: None, task });
                }

                Err(McpError::resource_not_found(format!("task not found: {}", task_id), None))
            }
        };
        item_impl.items.push(syn::parse2::<ImplItem>(get_info_fn)?);
    }

    if !has_method("get_task_result", &item_impl) {
        let get_result_fn = quote! {
            async fn get_task_result(
                &self,
                request: mcpkit_rs::model::GetTaskResultParam,
                _context: mcpkit_rs::service::RequestContext<mcpkit_rs::RoleServer>,
            ) -> Result<mcpkit_rs::model::GetTaskPayloadResult, McpError> {
                use std::time::Duration;
                let task_id = request.task_id.clone();

                loop {
                    // Scope the lock so we can await outside if needed
                    {
                        let mut processor = (#processor).lock().await;

                        if let Some(task_result) = processor.take_completed_result(&task_id) {
                            match task_result.result {
                                Ok(boxed) => {
                                    if let Some(tool) = boxed.as_any().downcast_ref::<mcpkit_rs::task_manager::ToolCallTaskResult>() {
                                        match &tool.result {
                                            Ok(call_tool) => {
                                                let value = ::serde_json::to_value(call_tool).unwrap_or(::serde_json::Value::Null);
                                                return Ok(mcpkit_rs::model::GetTaskPayloadResult::new(value));
                                            }
                                            Err(err) => return Err(McpError::internal_error(
                                                format!("task failed: {}", err),
                                                None,
                                            )),
                                        }
                                    } else {
                                        return Err(McpError::internal_error("unsupported task result transport", None));
                                    }
                                }
                                Err(err) => return Err(McpError::internal_error(
                                    format!("task execution error: {}", err),
                                    None,
                                )),
                            }
                        }

                        // Not completed yet: if not running, return not found
                        let running = processor.list_running();
                        if !running.iter().any(|id| id == &task_id) {
                            return Err(McpError::resource_not_found(format!("task not found: {}", task_id), None));
                        }
                    }

                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        };
        item_impl
            .items
            .push(syn::parse2::<ImplItem>(get_result_fn)?);
    }

    if !has_method("cancel_task", &item_impl) {
        let cancel_fn = quote! {
            async fn cancel_task(
                &self,
                request: mcpkit_rs::model::CancelTaskParam,
                _context: mcpkit_rs::service::RequestContext<mcpkit_rs::RoleServer>,
            ) -> Result<mcpkit_rs::model::CancelTaskResult, McpError> {
                use mcpkit_rs::task_manager::current_timestamp;
                let task_id = request.task_id;
                let mut processor = (#processor).lock().await;

                if processor.cancel_task(&task_id) {
                    let timestamp = current_timestamp();
                    let task = mcpkit_rs::model::Task::new(
                        task_id,
                        mcpkit_rs::model::TaskStatus::Cancelled,
                        timestamp.clone(),
                        timestamp,
                    );
                    return Ok(mcpkit_rs::model::CancelTaskResult { meta: None, task });
                }

                // If already completed, signal it's not cancellable
                let exists_completed = processor.peek_completed().iter().any(|r| r.descriptor.operation_id == task_id);
                if exists_completed {
                    return Err(McpError::invalid_request(format!("task already completed: {}", task_id), None));
                }

                Err(McpError::resource_not_found(format!("task not found: {}", task_id), None))
            }
        };
        item_impl.items.push(syn::parse2::<ImplItem>(cancel_fn)?);
    }

    Ok(item_impl.into_token_stream())
}
