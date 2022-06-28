// Tests located under data_fragment/tests.rs
#[cfg(test)]
mod tests;

// For additional docs see data_fragment/docs.md

use chrono::Utc;
use cid::multihash::{Code, MultihashDigest};
use cid::Cid;

const RAW: u64 = 0x55;

/// Represents partial or complete data identified by a CID
#[derive(Clone, Debug)]
pub struct DataFragment {
    /// We version the DataFragment every time it's mutated
    pub v: u32,
    /// MultiFormat CID: https://github.com/multiformats/cid
    pub cid: Cid,
    /// Timestamp indicating last mutation event (ns since epoch)
    pub timestamp: i64,
    /// Serialized data this fragment represents
    pub data: String,
    /// Is a stream is available for this data partial
    pub stream: bool,
    /// Represents wether the referenced stream is still alive
    alive: bool,
}

pub trait StaticFragment {
    fn default() -> DataFragment;
    fn update(&mut self, data: String) -> &mut Self;
    fn from(&mut self, data: String) -> DataFragment;
    fn _update_timestamp(&mut self);
}

pub trait LiveFragment {
    fn live(&mut self) -> &mut Self;
    fn kill(&mut self) -> &mut Self;
    fn is_alive(&mut self) -> bool;
}

impl StaticFragment for DataFragment {
    /// Returns a new DataFragment
    ///
    /// NOTICE: If you're here without using the Oracle you may be using it wrong.
    /// 
    /// # Arguments
    ///
    /// * `data` - An encoded string represending the data to be stored. Note the inital data is used to generate our CID
    /// * `live` - Optional boolean value representing if our data is a partial pointing to streamable data.
    ///
    /// # Examples
    ///
    /// ```
    /// // Creating a new DataFragment
    /// use data_fragment::DataFragment;
    /// DataFragment::new(Some(String::from("MockData")), None);
    /// ```
    /// 
    /// ```
    /// // Referencing a stream
    /// use data_fragment::DataFragment;
    /// let blob_partial = SomeBlob.slice().resize(32, 0).to_string();
    /// DataFragment::new(Some(blob), None);
    /// ```
    /// 
    
    fn default() -> DataFragment {
        let h = Code::Sha2_256.digest("default".as_bytes());

        DataFragment {
            v: 0,
            cid: Cid::new_v1(RAW, h),
            timestamp: 0,
            data: "".to_string(),
            stream: false,
            alive: false
        }
    }

    fn from(&mut self, data: String) -> DataFragment {
        // Generate a Multihash based on the content
        let h = Code::Sha2_256.digest(data.as_bytes());

        // Return the newley setup DataFragment
        self.cid = Cid::new_v1(RAW, h);
        self._update_timestamp();
        self.data = data;
        self.clone()
    }
    /// Internal method to update the timestamp, this happens any time the data is mutated
    fn _update_timestamp(&mut self) {
        let now = Utc::now();
        self.timestamp = now.timestamp_nanos();
    }

    /// Update the data stored inside the fragment.
    ///
    /// # Examples
    /// 
    /// ```
    /// use data_fragment::DataFragment;
    /// let fragment: DataFragment = DataFragment::new(Some("Inital data"), None);
    /// fragment.set_data(Some("New data"));
    /// ```
    fn update(&mut self, data: String) -> &mut Self {
        self.data = data;
        self.v += 1;
        self._update_timestamp();
        self
    }
}


impl LiveFragment for DataFragment {
    fn live(&mut self) -> &mut Self {
        self.alive = true;
        self._update_timestamp();
        self
    }
    /// Indicate that the associated stream is no longer alive
    /// 
    /// # Examples
    /// 
    /// ```
    /// use data_fragment::DataFragment;
    /// let fragment: DataFragment = DataFragment::new(Some("Stream Here"), None);
    /// fragment.kill();
    /// ```
    fn kill(&mut self) -> &mut Self {
        self.alive = false;
        self._update_timestamp();
        self
    }

    /// Check the status of the fragment
    ///
    /// # Examples
    /// 
    /// ```
    /// use data_fragment::DataFragment;
    /// let fragment: DataFragment = DataFragment::new(Some("Stream Here"), None);
    /// fragment.is_alive(); // false
    /// ```
    fn is_alive(&mut self) -> bool {
        self.alive
    }
}
