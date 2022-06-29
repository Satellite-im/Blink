use super::traits::FragmentError;

#[derive(Debug, Clone, Copy, strum_macros::Display)]
pub enum FragmentErrors {
    Generic,
    Collision,
}

pub struct FragmentErrorData {
    e: Option<FragmentErrors>,
    fatal: bool,
}

impl FragmentError for FragmentErrors {
    fn is_fatal(&self) -> bool {
        match self {
            FragmentErrors::Generic => false,
            FragmentErrors::Collision => true,
        }
    }
    fn get(&self) -> FragmentErrorData {
        match self {
            FragmentErrors::Generic => FragmentErrorData {
                e: Some(FragmentErrors::Generic),
                fatal: self.is_fatal(),
            },
            FragmentErrors::Collision => FragmentErrorData {
                e: Some(FragmentErrors::Generic),
                fatal: self.is_fatal(),
            },
        }
    }
}

impl FragmentError for FragmentErrorData {
    fn is_fatal(&self) -> bool {
        self.fatal
    }
    fn get(&self) -> FragmentErrorData {
        *self
    }
}

impl From<FragmentErrors> for FragmentErrorData {
    fn from(e: FragmentErrors) -> FragmentErrorData {
        FragmentErrorData {
            e: Some(e),
            fatal: false,
        }
    }
}

impl Default for FragmentErrorData {
    fn default() -> Self {
        FragmentErrorData {
            e: Some(FragmentErrors::Generic),
            fatal: false,
        }
    }
}
