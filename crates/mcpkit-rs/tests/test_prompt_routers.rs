#![cfg(all(feature = "schemars", feature = "macros"))]

use std::collections::HashMap;

use futures::future::BoxFuture;
use mcpkit_rs::{
    ServerHandler,
    handler::server::wrapper::Parameters,
    model::{GetPromptResult, PromptMessage, PromptMessageRole},
};

#[derive(Debug, Default)]
pub struct TestHandler<T: 'static = ()> {
    pub _marker: std::marker::PhantomData<fn(*const T)>,
}

impl<T: 'static> ServerHandler for TestHandler<T> {}

#[derive(Debug, schemars::JsonSchema, serde::Deserialize, serde::Serialize)]
pub struct Request {
    pub fields: HashMap<String, String>,
}

#[derive(Debug, schemars::JsonSchema, serde::Deserialize, serde::Serialize)]
pub struct Sum {
    pub a: i32,
    pub b: i32,
}

#[mcpkit_rs::prompt_router(router = "test_router")]
impl<T> TestHandler<T> {
    #[mcpkit_rs::prompt]
    async fn async_method(
        &self,
        Parameters(Request { fields }): Parameters<Request>,
    ) -> Vec<PromptMessage> {
        drop(fields);
        vec![PromptMessage::new_text(
            PromptMessageRole::Assistant,
            "Async method response",
        )]
    }

    #[mcpkit_rs::prompt]
    fn sync_method(
        &self,
        Parameters(Request { fields }): Parameters<Request>,
    ) -> Vec<PromptMessage> {
        drop(fields);
        vec![PromptMessage::new_text(
            PromptMessageRole::Assistant,
            "Sync method response",
        )]
    }
}

#[mcpkit_rs::prompt]
async fn async_function(Parameters(Request { fields }): Parameters<Request>) -> Vec<PromptMessage> {
    drop(fields);
    vec![PromptMessage::new_text(
        PromptMessageRole::Assistant,
        "Async function response",
    )]
}

#[mcpkit_rs::prompt]
fn async_function2<T>(_callee: &TestHandler<T>) -> BoxFuture<'_, GetPromptResult> {
    Box::pin(async move {
        GetPromptResult::new(vec![PromptMessage::new_text(
            PromptMessageRole::Assistant,
            "Async function 2 response",
        )])
        .with_description("Async function 2")
    })
}

#[test]
fn test_prompt_router() {
    let test_prompt_router = TestHandler::<()>::test_router()
        .with_route(
            mcpkit_rs::handler::server::router::prompt::PromptRoute::new_dyn(
                async_function_prompt_attr(),
                |mut context| {
                    Box::pin(async move {
                        use mcpkit_rs::handler::server::{
                            common::FromContextPart, prompt::IntoGetPromptResult,
                        };
                        let params = Parameters::<Request>::from_context_part(&mut context)?;
                        let result = async_function(params).await;
                        result.into_get_prompt_result()
                    })
                },
            ),
        )
        .with_route(
            mcpkit_rs::handler::server::router::prompt::PromptRoute::new_dyn(
                async_function2_prompt_attr(),
                |context| {
                    Box::pin(async move {
                        use mcpkit_rs::handler::server::prompt::IntoGetPromptResult;
                        let result = async_function2(context.server).await;
                        result.into_get_prompt_result()
                    })
                },
            ),
        );
    let prompts = test_prompt_router.list_all();
    assert_eq!(prompts.len(), 4);
}

#[test]
fn test_prompt_router_list_all_is_sorted() {
    let router = TestHandler::<()>::test_router()
        .with_route(
            mcpkit_rs::handler::server::router::prompt::PromptRoute::new_dyn(
                async_function_prompt_attr(),
                |mut context| {
                    Box::pin(async move {
                        use mcpkit_rs::handler::server::{
                            common::FromContextPart, prompt::IntoGetPromptResult,
                        };
                        let params = Parameters::<Request>::from_context_part(&mut context)?;
                        let result = async_function(params).await;
                        result.into_get_prompt_result()
                    })
                },
            ),
        )
        .with_route(
            mcpkit_rs::handler::server::router::prompt::PromptRoute::new_dyn(
                async_function2_prompt_attr(),
                |context| {
                    Box::pin(async move {
                        use mcpkit_rs::handler::server::prompt::IntoGetPromptResult;
                        let result = async_function2(context.server).await;
                        result.into_get_prompt_result()
                    })
                },
            ),
        );
    let prompts = router.list_all();
    let names: Vec<&str> = prompts.iter().map(|p| p.name.as_ref()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(
        names, sorted,
        "list_all() should return prompts sorted alphabetically by name"
    );
}
