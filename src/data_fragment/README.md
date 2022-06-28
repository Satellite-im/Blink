# DataFragment

## Notice: Docs are in progress, a lot of the structure and wording is evolving live.

### RTD 
Real-time Data ([RTD](https://en.wikipedia.org/wiki/Real-time_data)) will always be formatted in the same way. Blink provides a struct to layout this design called a DataFragment. A data fragment sending pub-sub type mesasges will contain a CID used to reference the data being sent. Data should be serialized BEFORE it's added to the DataFragment. 

Blink supports updating the DataFragment through the `set_data` function, however you should **never** mutate the fragment directly. The CID will remain the same for data that has been updated, but the version will be bumped every time it's mutated. Additionally the timestamp is updated any time a mutation occurs within the fragment.

Streams will generate the CID based on the first blob of data recieved. This will be used as a pointer to access the data stream later, consider the DataFragment as a notice that live data is ready for you to consume. You can access it through the [Oracle](https://tbd.tbd), using the `stream` method, passing in the CID.

Fragments will be emitted to designated peers on the network thus it's crutial for us to maintain a strict data standard. It is recommended that the serialized data is also standardized wherever it's being generated.

```rust
/// Represents partial or complete data identified by a CID
#[derive(Clone, Debug)]
pub(crate) struct DataFragment {
    /// We version the DataFragment every time it's mutated
    pub v: u32,
    /// MultiFormat CID: https://github.com/multiformats/cid
    pub cid: Cid,
    /// Timestamp indicating last mutation event
    pub timestamp: i64,
    /// Serialized data this fragment represents
    pub data: String,
    /// Is a stream is available for this data partial
    pub stream: bool,
}

```

In practice you should not need to interface directly with DataFragments besides updating data. The Conflux is in charge of creating and organizing fragments. There are major benifits to letting the Conflux manage your data including caching and automatic distrobution to the network.