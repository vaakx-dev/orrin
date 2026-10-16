use serde::Serialize;
use serde::de::DeserializeOwned;

pub trait Request: Serialize + DeserializeOwned + Send + 'static {
    const NAME: &'static str;
    type Output: Serialize + DeserializeOwned + Send + 'static;
}

macro_rules! requests {
    ($($input:ty => $name:literal : $output:ty),* $(,)?) => {
        $(
            impl $crate::api::Request for $input {
                const NAME: &'static str = $name;
                type Output = $output;
            }
        )*

        pub const REQUESTS: &[&str] = &[$($name),*];
    };
}

pub(crate) use requests;
