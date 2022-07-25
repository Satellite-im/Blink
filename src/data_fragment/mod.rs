// Tests located under data_fragment/tests.rs
pub(crate) mod errors;
#[cfg(test)]
mod tests;
pub(crate) mod traits;

// For additional docs see data_fragment/docs.md

use chrono::Utc;
use cid::multihash::{Code, MultihashDigest};
use cid::Cid;

use self::traits::{Fragment, FragmentAccessor, LiveFragment};

const RAW: u64 = 0x55;

/// Represents partial or complete data identified by a CID
#[derive(Clone, Debug)]
pub struct DataFragment {
    /// We version the DataFragment every time it's mutated
    pub v: i32,
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

impl Default for DataFragment {
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
            // A version of -1 indicates the fragment has not yet been initalized
            v: -1,
            cid: Cid::new_v1(RAW, h),
            timestamp: 0,
            data: String::new(),
            stream: false,
            alive: false,
        }
    }
}

impl From<String> for DataFragment {
    /// Generate a new CID and store our inital data
    fn from(data: String) -> Self {
        let h = Code::Sha2_256.digest(data.as_bytes());

        DataFragment {
            v: 0,
            cid: Cid::new_v1(RAW, h),
            timestamp: Utc::now().timestamp_nanos(),
            data,
            stream: false,
            alive: false,
        }
    }
}

impl FragmentAccessor for DataFragment {
    fn get(&self) -> &DataFragment {
        self
    }
    /// Update the data stored inside the fragment.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::data_fragment::{DataFragment};
    /// use crate::data_fragment::traits::FragmentAccessor;
    /// let fragment: DataFragment = DataFragment::from(Some("Inital data"), None);
    /// fragment.set("New data".to_string());
    /// ```
    fn set(&mut self, data: Option<String>) {
        self.data = data.unwrap_or(String::new());
        self.v += 1;
        // If this is the first data being entered into the fragment, we need to generate a CID
        if self.v == 0 {
            self._build_cid();
        }
        self._update_timestamp();
    }
}

impl Fragment for DataFragment {
    /// Internal method to update the timestamp, this happens any time the data is mutated
    fn _update_timestamp(&mut self) {
        let now = Utc::now();
        self.timestamp = now.timestamp_nanos();
    }

    fn _build_cid(&mut self) {
        // Generate a Multihash based on the content
        let h = Code::Sha2_256.digest(self.data.as_bytes());
        self.cid = Cid::new_v1(RAW, h);
    }
}

impl LiveFragment for DataFragment {
    fn wake(&mut self) -> &mut Self {
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
