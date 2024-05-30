mod into_anyhow {
    use gitbutler_core::error::{into_anyhow, Code, Context, ErrorWithContext};

    #[test]
    fn code_as_context() {
        #[derive(thiserror::Error, Debug)]
        #[error("err")]
        struct Error(Code);

        impl ErrorWithContext for Error {
            fn context(&self) -> Option<Context> {
                Context::from(self.0).into()
            }
        }

        let err = into_anyhow(Error(Code::Validation));
        let ctx = err.downcast_ref::<Context>().unwrap();
        assert_eq!(ctx.code, Code::Validation, "the context is attached");
        assert_eq!(
            ctx.message, None,
            "there is no message when context was created from bare code"
        );
    }

    #[test]
    fn nested_code_as_context() {
        #[derive(thiserror::Error, Debug)]
        #[error("Err")]
        struct Inner(Code);

        #[derive(thiserror::Error, Debug)]
        #[error(transparent)]
        struct Outer(#[from] Inner);

        impl ErrorWithContext for Outer {
            fn context(&self) -> Option<Context> {
                Context::from(self.0 .0).into()
            }
        }

        let err = into_anyhow(Outer::from(Inner(Code::Validation)));
        let ctx = err.downcast_ref::<Context>().unwrap();
        assert_eq!(
            ctx.code,
            Code::Validation,
            "there is no magic here, it's all about manually implementing the nesting :/"
        );
    }
}
