use std::{
    borrow::Cow,
    future::{Future, Ready},
    marker::PhantomData,
};

use futures::future::{BoxFuture, FutureExt};
use serde::de::DeserializeOwned;

use super::common::{AsRequestContext, FromContextPart};
pub use super::{
    common::{Extension, RequestId, schema_for_output, schema_for_type},
    router::tool::{ToolRoute, ToolRouter},
};
use crate::{
    RoleServer,
    handler::server::wrapper::Parameters,
    model::{CallToolRequestParam, CallToolResult, IntoContents, JsonObject},
    service::RequestContext,
};

/// Deserialize a JSON object into a type
pub fn parse_json_object<T: DeserializeOwned>(
    input: JsonObject,
) -> Result<T, crate::ErrorData> {
    serde_json::from_value(serde_json::Value::Object(input)).map_err(|e| {
        crate::ErrorData::invalid_params(
            format!("failed to deserialize parameters: {error}", error = e),
            None,
        )
    })
}
pub struct ToolCallContext<'s, S> {
    pub request_context: RequestContext<RoleServer>,
    pub service: &'s S,
    pub name: Cow<'static, str>,
    pub arguments: Option<JsonObject>,
    pub task: Option<JsonObject>,
}

impl<'s, S> ToolCallContext<'s, S> {
    pub fn new(
        service: &'s S,
        CallToolRequestParam {
            name,
            arguments,
            task,
        }: CallToolRequestParam,
        request_context: RequestContext<RoleServer>,
    ) -> Self {
        Self {
            request_context,
            service,
            name,
            arguments,
            task,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn request_context(&self) -> &RequestContext<RoleServer> {
        &self.request_context
    }
}

impl<S> AsRequestContext for ToolCallContext<'_, S> {
    fn as_request_context(&self) -> &RequestContext<RoleServer> {
        &self.request_context
    }

    fn as_request_context_mut(&mut self) -> &mut RequestContext<RoleServer> {
        &mut self.request_context
    }
}

pub trait IntoCallToolResult {
    fn into_call_tool_result(self) -> Result<CallToolResult, crate::ErrorData>;
}

impl<T: IntoContents> IntoCallToolResult for T {
    fn into_call_tool_result(self) -> Result<CallToolResult, crate::ErrorData> {
        Ok(CallToolResult::success(self.into_contents()))
    }
}

impl<T: IntoContents, E: IntoContents> IntoCallToolResult for Result<T, E> {
    fn into_call_tool_result(self) -> Result<CallToolResult, crate::ErrorData> {
        match self {
            Ok(value) => Ok(CallToolResult::success(value.into_contents())),
            Err(error) => Ok(CallToolResult::error(error.into_contents())),
        }
    }
}

impl<T: IntoCallToolResult> IntoCallToolResult for Result<T, crate::ErrorData> {
    fn into_call_tool_result(self) -> Result<CallToolResult, crate::ErrorData> {
        match self {
            Ok(value) => value.into_call_tool_result(),
            Err(error) => Err(error),
        }
    }
}

pin_project_lite::pin_project! {
    #[project = IntoCallToolResultFutProj]
    pub enum IntoCallToolResultFut<F, R> {
        Pending {
            #[pin]
            fut: F,
            _marker: PhantomData<R>,
        },
        Ready {
            #[pin]
            result: Ready<Result<CallToolResult, crate::ErrorData>>,
        }
    }
}

impl<F, R> Future for IntoCallToolResultFut<F, R>
where
    F: Future<Output = R>,
    R: IntoCallToolResult,
{
    type Output = Result<CallToolResult, crate::ErrorData>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.project() {
            IntoCallToolResultFutProj::Pending { fut, _marker } => {
                fut.poll(cx).map(IntoCallToolResult::into_call_tool_result)
            }
            IntoCallToolResultFutProj::Ready { result } => result.poll(cx),
        }
    }
}

impl IntoCallToolResult for Result<CallToolResult, crate::ErrorData> {
    fn into_call_tool_result(self) -> Result<CallToolResult, crate::ErrorData> {
        self
    }
}

pub trait CallToolHandler<S, A> {
    fn call(
        self,
        context: ToolCallContext<'_, S>,
    ) -> BoxFuture<'_, Result<CallToolResult, crate::ErrorData>>;
}

pub type DynCallToolHandler<S> = dyn for<'s> Fn(
        ToolCallContext<'s, S>,
    ) -> BoxFuture<'s, Result<CallToolResult, crate::ErrorData>>
    + Send
    + Sync;

// Tool-specific extractor for tool name
pub struct ToolName(pub Cow<'static, str>);

impl<S> FromContextPart<ToolCallContext<'_, S>> for ToolName {
    fn from_context_part(
        context: &mut ToolCallContext<S>,
    ) -> Result<Self, crate::ErrorData> {
        Ok(Self(context.name.clone()))
    }
}

// Special implementation for Parameters that handles tool arguments
impl<S, P> FromContextPart<ToolCallContext<'_, S>> for Parameters<P>
where
    P: DeserializeOwned,
{
    fn from_context_part(
        context: &mut ToolCallContext<S>,
    ) -> Result<Self, crate::ErrorData> {
        let arguments = context.arguments.take().unwrap_or_default();
        let value: P = serde_json::from_value(serde_json::Value::Object(arguments))
            .map_err(|e| {
                crate::ErrorData::invalid_params(
                    format!("failed to deserialize parameters: {error}", error = e),
                    None,
                )
            })?;
        Ok(Parameters(value))
    }
}

// Special implementation for JsonObject that takes tool arguments
impl<S> FromContextPart<ToolCallContext<'_, S>> for JsonObject {
    fn from_context_part(
        context: &mut ToolCallContext<S>,
    ) -> Result<Self, crate::ErrorData> {
        let object = context.arguments.take().unwrap_or_default();
        Ok(object)
    }
}

impl<'s, S> ToolCallContext<'s, S> {
    pub fn invoke<H, A>(
        self,
        h: H,
    ) -> BoxFuture<'s, Result<CallToolResult, crate::ErrorData>>
    where
        H: CallToolHandler<S, A>,
    {
        h.call(self)
    }
}
#[allow(clippy::type_complexity)]
pub struct AsyncAdapter<P, Fut, R>(PhantomData<fn(P) -> fn(Fut) -> R>);
pub struct SyncAdapter<P, R>(PhantomData<fn(P) -> R>);
// #[allow(clippy::type_complexity)]
pub struct AsyncMethodAdapter<P, R>(PhantomData<fn(P) -> R>);
pub struct SyncMethodAdapter<P, R>(PhantomData<fn(P) -> R>);

macro_rules! impl_for {
    ($($T: ident)*) => {
        impl_for!([] [$($T)*]);
    };
    // finished
    ([$($Tn: ident)*] []) => {
        impl_for!(@impl $($Tn)*);
    };
    ([$($Tn: ident)*] [$Tn_1: ident $($Rest: ident)*]) => {
        impl_for!(@impl $($Tn)*);
        impl_for!([$($Tn)* $Tn_1] [$($Rest)*]);
    };
    (@impl $($Tn: ident)*) => {
        impl<$($Tn,)* S, F,  R> CallToolHandler<S, AsyncMethodAdapter<($($Tn,)*), R>> for F
        where
            $(
                $Tn: for<'a> FromContextPart<ToolCallContext<'a, S>> ,
            )*
            F: FnOnce(&S, $($Tn,)*) -> BoxFuture<'_, R>,

            // Need RTN support here(I guess), https://github.com/rust-lang/rust/pull/138424
            // Fut: Future<Output = R> + Send + 'a,
            R: IntoCallToolResult + Send + 'static,
            S: Send + Sync + 'static,
        {
            #[allow(unused_variables, non_snake_case, unused_mut)]
            fn call(
                self,
                mut context: ToolCallContext<'_, S>,
            ) -> BoxFuture<'_, Result<CallToolResult, crate::ErrorData>>{
                $(
                    let result = $Tn::from_context_part(&mut context);
                    let $Tn = match result {
                        Ok(value) => value,
                        Err(e) => return std::future::ready(Err(e)).boxed(),
                    };
                )*
                let service = context.service;
                let fut = self(service, $($Tn,)*);
                async move {
                    let result = fut.await;
                    result.into_call_tool_result()
                }.boxed()
            }
        }

        impl<$($Tn,)* S, F, Fut, R> CallToolHandler<S, AsyncAdapter<($($Tn,)*), Fut, R>> for F
        where
            $(
                $Tn: for<'a> FromContextPart<ToolCallContext<'a, S>> ,
            )*
            F: FnOnce($($Tn,)*) -> Fut + Send + ,
            Fut: Future<Output = R> + Send + 'static,
            R: IntoCallToolResult + Send + 'static,
            S: Send + Sync,
        {
            #[allow(unused_variables, non_snake_case, unused_mut)]
            fn call(
                self,
                mut context: ToolCallContext<S>,
            ) -> BoxFuture<'static, Result<CallToolResult, crate::ErrorData>>{
                $(
                    let result = $Tn::from_context_part(&mut context);
                    let $Tn = match result {
                        Ok(value) => value,
                        Err(e) => return std::future::ready(Err(e)).boxed(),
                    };
                )*
                let fut = self($($Tn,)*);
                async move {
                    let result = fut.await;
                    result.into_call_tool_result()
                }.boxed()
            }
        }

        impl<$($Tn,)* S, F, R> CallToolHandler<S, SyncMethodAdapter<($($Tn,)*), R>> for F
        where
            $(
                $Tn: for<'a> FromContextPart<ToolCallContext<'a, S>> + ,
            )*
            F: FnOnce(&S, $($Tn,)*) -> R + Send + ,
            R: IntoCallToolResult + Send + ,
            S: Send + Sync,
        {
            #[allow(unused_variables, non_snake_case, unused_mut)]
            fn call(
                self,
                mut context: ToolCallContext<S>,
            ) -> BoxFuture<'static, Result<CallToolResult, crate::ErrorData>> {
                $(
                    let result = $Tn::from_context_part(&mut context);
                    let $Tn = match result {
                        Ok(value) => value,
                        Err(e) => return std::future::ready(Err(e)).boxed(),
                    };
                )*
                std::future::ready(self(context.service, $($Tn,)*).into_call_tool_result()).boxed()
            }
        }

        impl<$($Tn,)* S, F, R> CallToolHandler<S, SyncAdapter<($($Tn,)*), R>> for F
        where
            $(
                $Tn: for<'a> FromContextPart<ToolCallContext<'a, S>> + ,
            )*
            F: FnOnce($($Tn,)*) -> R + Send + ,
            R: IntoCallToolResult + Send + ,
            S: Send + Sync,
        {
            #[allow(unused_variables, non_snake_case, unused_mut)]
            fn call(
                self,
                mut context: ToolCallContext<S>,
            ) -> BoxFuture<'static, Result<CallToolResult, crate::ErrorData>>  {
                $(
                    let result = $Tn::from_context_part(&mut context);
                    let $Tn = match result {
                        Ok(value) => value,
                        Err(e) => return std::future::ready(Err(e)).boxed(),
                    };
                )*
                std::future::ready(self($($Tn,)*).into_call_tool_result()).boxed()
            }
        }
    };
}
impl_for!(T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15);

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::model::NumberOrString;

    #[derive(Debug, Clone)]
    struct TestService {
        #[allow(dead_code)]
        value: String,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct TestParams {
        message: String,
        count: i32,
    }

    #[tokio::test]
    async fn test_parse_json_object_valid() {
        let mut json = JsonObject::new();
        json.insert("message".to_string(), json!("hello"));
        json.insert("count".to_string(), json!(42));

        let result: Result<TestParams, _> = parse_json_object(json);
        assert!(result.is_ok());
        let params = result.unwrap();
        assert_eq!(params.message, "hello");
        assert_eq!(params.count, 42);
    }

    #[tokio::test]
    async fn test_parse_json_object_invalid() {
        let mut json = JsonObject::new();
        json.insert("message".to_string(), json!("hello"));
        // Missing required field 'count'

        let result: Result<TestParams, _> = parse_json_object(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("failed to deserialize"));
    }

    #[tokio::test]
    async fn test_parse_json_object_type_mismatch() {
        let mut json = JsonObject::new();
        json.insert("message".to_string(), json!("hello"));
        json.insert("count".to_string(), json!("not a number")); // Wrong type

        let result: Result<TestParams, _> = parse_json_object(json);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_into_call_tool_result_string() {
        let result = "success".to_string().into_call_tool_result();
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert_eq!(tool_result.is_error, Some(false));
        assert_eq!(tool_result.content.len(), 1);
        if let Some(text) = tool_result.content[0].as_text() {
            assert_eq!(text.text, "success");
        } else {
            panic!("Expected text content");
        }
    }

    #[tokio::test]
    async fn test_into_call_tool_result_ok_variant() {
        let result: Result<String, String> = Ok("success".to_string());
        let tool_result = result.into_call_tool_result().unwrap();
        assert_eq!(tool_result.is_error, Some(false));
        assert_eq!(tool_result.content.len(), 1);
    }

    #[tokio::test]
    async fn test_into_call_tool_result_err_variant() {
        let result: Result<String, String> = Err("error".to_string());
        let tool_result = result.into_call_tool_result().unwrap();
        assert_eq!(tool_result.is_error, Some(true));
        assert_eq!(tool_result.content.len(), 1);
        if let Some(text) = tool_result.content[0].as_text() {
            assert_eq!(text.text, "error");
        } else {
            panic!("Expected text content");
        }
    }

    #[tokio::test]
    async fn test_into_call_tool_result_error_data() {
        let error = crate::ErrorData::invalid_params("bad params".to_string(), None);
        let result: Result<String, crate::ErrorData> = Err(error);
        let tool_result = result.into_call_tool_result();
        assert!(tool_result.is_err());
        assert!(tool_result.unwrap_err().message.contains("bad params"));
    }

    #[tokio::test]
    async fn test_tool_name_extraction() {
        let service = TestService {
            value: "test".to_string(),
        };
        let request_context = RequestContext {
            peer:
                crate::service::Peer::new(
                    std::sync::Arc::new(
                        crate::service::AtomicU32RequestIdProvider::default(),
                    ),
                    None,
                )
                .0,
            ct: CancellationToken::new(),
            id: NumberOrString::Number(1),
            meta: Default::default(),
            extensions: Default::default(),
        };

        let mut context = ToolCallContext::new(
            &service,
            CallToolRequestParam {
                name: "test_tool".into(),
                arguments: None,
                task: None,
            },
            request_context,
        );

        let tool_name = ToolName::from_context_part(&mut context).unwrap();
        assert_eq!(tool_name.0, "test_tool");
    }

    #[tokio::test]
    async fn test_parameters_extraction() {
        let service = TestService {
            value: "test".to_string(),
        };
        let mut args = JsonObject::new();
        args.insert("message".to_string(), json!("hello"));
        args.insert("count".to_string(), json!(42));

        let request_context = RequestContext {
            peer:
                crate::service::Peer::new(
                    std::sync::Arc::new(
                        crate::service::AtomicU32RequestIdProvider::default(),
                    ),
                    None,
                )
                .0,
            ct: CancellationToken::new(),
            id: NumberOrString::Number(1),
            meta: Default::default(),
            extensions: Default::default(),
        };

        let mut context = ToolCallContext::new(
            &service,
            CallToolRequestParam {
                name: "test_tool".into(),
                arguments: Some(args),
                task: None,
            },
            request_context,
        );

        let params: Parameters<TestParams> =
            Parameters::from_context_part(&mut context).unwrap();
        assert_eq!(params.0.message, "hello");
        assert_eq!(params.0.count, 42);
        // Arguments should be consumed
        assert!(context.arguments.is_none());
    }

    #[tokio::test]
    async fn test_parameters_extraction_empty() {
        let service = TestService {
            value: "test".to_string(),
        };

        let request_context = RequestContext {
            peer:
                crate::service::Peer::new(
                    std::sync::Arc::new(
                        crate::service::AtomicU32RequestIdProvider::default(),
                    ),
                    None,
                )
                .0,
            ct: CancellationToken::new(),
            id: NumberOrString::Number(1),
            meta: Default::default(),
            extensions: Default::default(),
        };

        let mut context = ToolCallContext::new(
            &service,
            CallToolRequestParam {
                name: "test_tool".into(),
                arguments: None,
                task: None,
            },
            request_context,
        );

        // Should use empty object when no arguments
        let json_obj: JsonObject = JsonObject::from_context_part(&mut context).unwrap();
        assert!(json_obj.is_empty());
    }

    #[tokio::test]
    async fn test_async_handler_success() {
        async fn async_tool(params: Parameters<TestParams>) -> String {
            format!("{} x {}", params.0.message, params.0.count)
        }

        let service = TestService {
            value: "test".to_string(),
        };
        let mut args = JsonObject::new();
        args.insert("message".to_string(), json!("hello"));
        args.insert("count".to_string(), json!(3));

        let request_context = RequestContext {
            peer:
                crate::service::Peer::new(
                    std::sync::Arc::new(
                        crate::service::AtomicU32RequestIdProvider::default(),
                    ),
                    None,
                )
                .0,
            ct: CancellationToken::new(),
            id: NumberOrString::Number(1),
            meta: Default::default(),
            extensions: Default::default(),
        };

        let context = ToolCallContext::new(
            &service,
            CallToolRequestParam {
                name: "async_tool".into(),
                arguments: Some(args),
                task: None,
            },
            request_context,
        );

        let result = context.invoke(async_tool).await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert_eq!(tool_result.is_error, Some(false));
        if let Some(text) = tool_result.content[0].as_text() {
            assert_eq!(text.text, "hello x 3");
        } else {
            panic!("Expected text content");
        }
    }

    #[tokio::test]
    async fn test_sync_handler_success() {
        fn sync_tool(params: Parameters<TestParams>) -> String {
            format!("{} x {}", params.0.message, params.0.count)
        }

        let service = TestService {
            value: "test".to_string(),
        };
        let mut args = JsonObject::new();
        args.insert("message".to_string(), json!("test"));
        args.insert("count".to_string(), json!(5));

        let request_context = RequestContext {
            peer:
                crate::service::Peer::new(
                    std::sync::Arc::new(
                        crate::service::AtomicU32RequestIdProvider::default(),
                    ),
                    None,
                )
                .0,
            ct: CancellationToken::new(),
            id: NumberOrString::Number(1),
            meta: Default::default(),
            extensions: Default::default(),
        };

        let context = ToolCallContext::new(
            &service,
            CallToolRequestParam {
                name: "sync_tool".into(),
                arguments: Some(args),
                task: None,
            },
            request_context,
        );

        let result = context.invoke(sync_tool).await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert_eq!(tool_result.is_error, Some(false));
        if let Some(text) = tool_result.content[0].as_text() {
            assert_eq!(text.text, "test x 5");
        } else {
            panic!("Expected text content");
        }
    }

    #[tokio::test]
    async fn test_handler_with_result_error() {
        async fn failing_tool(_params: Parameters<TestParams>) -> Result<String, String> {
            Err("Tool execution failed".to_string())
        }

        let service = TestService {
            value: "test".to_string(),
        };
        let mut args = JsonObject::new();
        args.insert("message".to_string(), json!("test"));
        args.insert("count".to_string(), json!(1));

        let request_context = RequestContext {
            peer:
                crate::service::Peer::new(
                    std::sync::Arc::new(
                        crate::service::AtomicU32RequestIdProvider::default(),
                    ),
                    None,
                )
                .0,
            ct: CancellationToken::new(),
            id: NumberOrString::Number(1),
            meta: Default::default(),
            extensions: Default::default(),
        };

        let context = ToolCallContext::new(
            &service,
            CallToolRequestParam {
                name: "failing_tool".into(),
                arguments: Some(args),
                task: None,
            },
            request_context,
        );

        let result = context.invoke(failing_tool).await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert_eq!(tool_result.is_error, Some(true));
        if let Some(text) = tool_result.content[0].as_text() {
            assert_eq!(text.text, "Tool execution failed");
        } else {
            panic!("Expected text content");
        }
    }

    #[tokio::test]
    async fn test_handler_with_json_string_output() {
        async fn json_tool(params: Parameters<TestParams>) -> String {
            let result = json!({
                "message": params.0.message,
                "count": params.0.count,
                "computed": params.0.count * 2
            });
            result.to_string()
        }

        let service = TestService {
            value: "test".to_string(),
        };
        let mut args = JsonObject::new();
        args.insert("message".to_string(), json!("hello"));
        args.insert("count".to_string(), json!(10));

        let request_context = RequestContext {
            peer:
                crate::service::Peer::new(
                    std::sync::Arc::new(
                        crate::service::AtomicU32RequestIdProvider::default(),
                    ),
                    None,
                )
                .0,
            ct: CancellationToken::new(),
            id: NumberOrString::Number(1),
            meta: Default::default(),
            extensions: Default::default(),
        };

        let context = ToolCallContext::new(
            &service,
            CallToolRequestParam {
                name: "json_tool".into(),
                arguments: Some(args),
                task: None,
            },
            request_context,
        );

        let result = context.invoke(json_tool).await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert_eq!(tool_result.is_error, Some(false));
        if let Some(text) = tool_result.content[0].as_text() {
            let parsed: serde_json::Value = serde_json::from_str(&text.text).unwrap();
            assert_eq!(parsed["message"], "hello");
            assert_eq!(parsed["count"], 10);
            assert_eq!(parsed["computed"], 20);
        } else {
            panic!("Expected text content");
        }
    }
}
