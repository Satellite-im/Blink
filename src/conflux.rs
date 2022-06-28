use crate::data_fragment::{DataFragment, StaticFragment, LiveFragment};

pub(crate) struct Conflux {
    fragments: Vec<DataFragment>,

}

pub trait ConfluxTrait {
    
}

impl Conflux {
    pub fn close_stream<T: LiveFragment>(mut fragment: T) {
        fragment.kill();
    }
}