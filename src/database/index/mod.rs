mod negative;
mod positive;

pub(crate) use self::negative::Negative;
pub(crate) use self::positive::{Positive, PositiveBuilder};

use std::error::Error;
use std::io::Cursor;
use std::sync::Arc;

use fst::{IntoStreamer, Streamer};
use sdset::duo::DifferenceByKey;
use sdset::{Set, SetOperation};
use fst::Map;

use crate::data::{SharedData, DocIndexes};

#[derive(Default)]
pub struct Index {
    pub(crate) negative: Negative,
    pub(crate) positive: Positive,
}

impl Index {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Index, Box<Error>> {
        let len = bytes.len();
        Index::from_shared_bytes(Arc::new(bytes), 0, len)
    }

    pub fn from_shared_bytes(
        bytes: Arc<Vec<u8>>,
        offset: usize,
        len: usize,
    ) -> Result<Index, Box<Error>>
    {
        let data = SharedData::new(bytes, offset, len);
        let mut cursor = Cursor::new(data);

        let negative = Negative::from_cursor(&mut cursor)?;
        let positive = Positive::from_cursor(&mut cursor)?;
        Ok(Index { negative, positive })
    }

    pub fn write_to_bytes(&self, bytes: &mut Vec<u8>) {
        self.negative.write_to_bytes(bytes);
        self.positive.write_to_bytes(bytes);
    }

    pub fn merge(&self, other: &Index) -> Result<Index, Box<Error>> {
        if other.negative.is_empty() {
            let negative = Negative::default();
            let positive = self.positive.union(&other.positive)?;
            return Ok(Index { negative, positive })
        }

        let mut buffer = Vec::new();
        let mut builder = PositiveBuilder::memory();
        let mut stream = self.positive.into_stream();
        while let Some((key, indexes)) = stream.next() {
            let op = DifferenceByKey::new(indexes, &other.negative, |x| x.document_id, |x| *x);

            buffer.clear();
            op.extend_vec(&mut buffer);

            if !buffer.is_empty() {
                let indexes = Set::new_unchecked(&buffer);
                builder.insert(key, indexes)?;
            }
        }

        let positive = {
            let (map, indexes) = builder.into_inner()?;
            let map = Map::from_bytes(map)?;
            let indexes = DocIndexes::from_bytes(indexes)?;
            Positive::new(map, indexes)
        };

        let negative = Negative::default();
        let positive = positive.union(&other.positive)?;
        Ok(Index { negative, positive })
    }
}