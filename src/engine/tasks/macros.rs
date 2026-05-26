// src/engine/tasks/macros.rs
//
// The `define_encode_task!` macro generates encode-style task structs and
// their NAPI `Task` trait implementations. It eliminates ~30 lines of
// boilerplate per task (struct definition, `Send` assertion, `compute` /
// `resolve` / `reject` wiring) that was previously copy-pasted 7+ times.

/// Generate a task struct with `ctx: TaskContext` and optional extra fields,
/// plus its NAPI `Task` impl.
///
/// Syntax:
/// ```ignore
/// define_encode_task! {
///     /// doc comment
///     pub struct MyTask {
///         // extra fields (TaskContext is always included as `ctx`)
///         pub extra_field: Type,
///     }
///     napi {
///         type Output = OutputType;
///         type JsValue = JsType;
///         compute(self) { ... }
///         resolve(self, env, output) { ... }
///     }
/// }
/// ```
macro_rules! define_encode_task {
    (
        $( #[$meta:meta] )*
        pub struct $name:ident {
            $( $( #[$fmeta:meta] )* pub $field:ident : $fty:ty ),* $(,)?
        }
        napi {
            type Output = $out:ty;
            type JsValue = $jsval:ty;
            compute($self_compute:ident) $compute_body:block
            resolve($self_resolve:ident, $env_resolve:ident, $output_resolve:ident) $resolve_body:block
        }
    ) => {
        $( #[$meta] )*
        pub struct $name {
            pub ctx: TaskContext,
            $( $( #[$fmeta] )* pub $field : $fty, )*
        }

        // Compile-time `Send` guarantee. NAPI's `AsyncTask` moves task structs
        // onto a worker thread for `compute()`, so any non-Send field would
        // surface as a runtime error rather than at build time. This assertion
        // catches it during compilation, before the napi attribute even
        // expands.
        const _: fn() = || {
            fn assert_send<T: Send>() {}
            assert_send::<$name>();
        };

        #[cfg(feature = "napi")]
        #[napi]
        impl Task for $name {
            type Output = $out;
            type JsValue = $jsval;

            fn compute(&mut $self_compute) -> Result<Self::Output> $compute_body

            fn resolve(&mut $self_resolve, $env_resolve: Env, $output_resolve: Self::Output) -> Result<Self::JsValue> $resolve_body

            fn reject(&mut self, env: Env, err: napi::Error) -> Result<Self::JsValue> {
                let lazy_err = self.ctx.take_stored_error(err);
                let napi_err = crate::error::napi_error_with_code(&env, lazy_err)?;
                Err(napi_err)
            }
        }
    };
}
