use super::{DataFragment, errors::FragmentErrorData};

pub trait FragmentAccessor {
    fn get(&self) -> &DataFragment;
    fn set(&mut self, data: Option<String>);
}

pub trait Fragment {
    fn _update_timestamp(&mut self);
    fn _build_cid(&mut self);
}

pub trait LiveFragment {
    fn wake(&mut self) -> &mut Self;
    fn kill(&mut self) -> &mut Self;
    fn is_alive(&mut self) -> bool;
}

pub trait FragmentError {
    fn is_fatal(&self) -> bool;
    fn get(&self) -> FragmentErrorData;
}
