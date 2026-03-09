use std::{borrow::Cow, future::Future, pin::Pin, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{
    ErrorData,
    handler::server::{
        common::schema_for_empty_input,
        tool::{schema_for_output, schema_for_type},
        wrapper::{Json, Parameters},
    },
    model::{Icon, JsonObject, Meta, ToolAnnotations, ToolExecution},
    schemars::JsonSchema,
};

/// Base trait to define attributes of a tool.
///
/// Tools implementing [`SyncTool`] or [`AsyncTool`] must implement this trait first.
///
/// All methods are consistent with fields of [`Tool`][crate::model::Tool].
pub trait ToolBase {
    /// Parameter type, will used in the invoke parameter of [`SyncTool`] or [`AsyncTool`] trait
    ///
    /// If the tool does not have any parameters, you **MUST** override [`input_schema`][Self::input_schema]
    /// method. See its documentation for more details.
    type Parameter: for<'de> Deserialize<'de> + JsonSchema + Send + Default + 'static;
    /// Output type, will used in the invoke output of [`SyncTool`] or [`AsyncTool`] trait
    ///
    /// If the tool does not have any output, you **MUST** override [`output_schema`][Self::output_schema]
    /// method. See its documentation for more details.
    type Output: Serialize + JsonSchema + Send + 'static;
    /// Error type, will used in the invoke output of [`SyncTool`] or [`AsyncTool`] trait
    type Error: Into<ErrorData> + Send + 'static;

    fn name() -> Cow<'static, str>;

    fn title() -> Option<String> {
        None
    }
    fn description() -> Option<Cow<'static, str>> {
        None
    }

    /// Json schema for tool input.
    ///
    /// The default implementation generates schema based on [`Self::Parameter`] type.
    ///
    /// If the tool does not have any parameters, you should override this methods to return [`None`],
    /// and when invoked, the parameter will get default values.
    fn input_schema() -> Option<Arc<JsonObject>> {
        Some(schema_for_type::<Parameters<Self::Parameter>>())
    }

    /// Json schema for tool output.
    ///
    /// The default implementation generates schema based on [`Self::Output`] type.
    ///
    /// If the tool does not have any output, you should override this methods to return [`None`].
    fn output_schema() -> Option<Arc<JsonObject>> {
        Some(schema_for_output::<Self::Output>().unwrap_or_else(|e| {
            panic!(
                "Invalid output schema for ToolBase::Output type `{0}`: {1}",
                std::any::type_name::<Self::Output>(),
                e,
            );
        }))
    }

    fn annotations() -> Option<ToolAnnotations> {
        None
    }
    fn execution() -> Option<ToolExecution> {
        None
    }
    fn icons() -> Option<Vec<Icon>> {
        None
    }
    fn meta() -> Option<Meta> {
        None
    }
}

/// Synchronous version of a tool.
///
/// Consider using [`AsyncTool`] if your workflow involves asynchronous operations.
/// Examples are shown in [the module-level documentation][crate::handler::server::router::tool].
pub trait SyncTool<S: Sync + Send + 'static>: ToolBase {
    fn invoke(service: &S, param: Self::Parameter) -> Result<Self::Output, Self::Error>;
}

/// Asynchronous version of a tool.
///
/// Consider using [`SyncTool`] if your workflow does not involve asynchronous operations.
/// Examples are shown in [the module-level documentation][crate::handler::server::router::tool].
pub trait AsyncTool<S: Sync + Send + 'static>: ToolBase {
    fn invoke(
        service: &S,
        param: Self::Parameter,
    ) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send;
}

pub(crate) fn tool_attribute<T: ToolBase>() -> crate::model::Tool {
    crate::model::Tool {
        name: T::name(),
        title: T::title(),
        description: T::description(),
        input_schema: T::input_schema().unwrap_or_else(schema_for_empty_input),
        output_schema: T::output_schema(),
        annotations: T::annotations(),
        execution: T::execution(),
        icons: T::icons(),
        meta: T::meta(),
    }
}

pub(crate) fn sync_tool_wrapper<S: Sync + Send + 'static, T: SyncTool<S>>(
    service: &S,
    Parameters(params): Parameters<T::Parameter>,
) -> Result<Json<T::Output>, ErrorData> {
    T::invoke(service, params).map(Json).map_err(Into::into)
}

pub(crate) fn sync_tool_wrapper_with_empty_params<S: Sync + Send + 'static, T: SyncTool<S>>(
    service: &S,
) -> Result<Json<T::Output>, ErrorData> {
    T::invoke(service, T::Parameter::default())
        .map(Json)
        .map_err(Into::into)
}

#[expect(clippy::type_complexity)]
pub(crate) fn async_tool_wrapper<S: Sync + Send + 'static, T: AsyncTool<S>>(
    service: &S,
    Parameters(params): Parameters<T::Parameter>,
) -> Pin<Box<dyn Future<Output = Result<Json<T::Output>, ErrorData>> + Send + '_>> {
    Box::pin(async move {
        T::invoke(service, params)
            .await
            .map(Json)
            .map_err(Into::into)
    })
}

#[expect(clippy::type_complexity)]
pub(crate) fn async_tool_wrapper_with_empty_params<S: Sync + Send + 'static, T: AsyncTool<S>>(
    service: &S,
) -> Pin<Box<dyn Future<Output = Result<Json<T::Output>, ErrorData>> + Send + '_>> {
    Box::pin(async move {
        T::invoke(service, T::Parameter::default())
            .await
            .map(Json)
            .map_err(Into::into)
    })
}
