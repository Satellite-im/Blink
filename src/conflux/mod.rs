use crate::data_fragment::{DataFragment, traits::{LiveFragment, Fragment, FragmentError}, errors::{FragmentErrors}};

pub(crate) struct Conflux {
    fragments: Vec<DataFragment>,

}

pub trait ConfluxTrait {
    
}

impl Conflux {
    pub fn close_stream<T: LiveFragment>(mut fragment: T) {
        fragment.kill();
    }

    fn add_fragment<T: Fragment, LiveFragment, E: FragmentError>(&mut self, fragment: T) -> Result<T, E> {
        let collides = self.check_collision(fragment); 
        
        match collides {
            Ok(f) => {
                self.fragments.push(fragment);
                Ok(fragment)
            },
            Err(e) => Err(e),
        }
    }

    fn check_collision<T: Fragment, LiveFragment, E: FragmentError>(self, fragment: &T) -> Result<T, E>{
        let colliding_fragments: Vec<&DataFragment> = self.fragments
            .iter()
            .filter(|x: &&DataFragment| *x.cid.to_string() == fragment.cid).collect::<Vec<_>>();
       
        if colliding_fragments.len() > 0 {
            return Err(FragmentError::from(FragmentErrors::Collision));
        } else {
            return Ok(*fragment);
        }
    }
}