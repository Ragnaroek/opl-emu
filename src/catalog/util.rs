pub struct DataReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl DataReader<'_> {
    pub fn new(data: &[u8]) -> DataReader<'_> {
        DataReader::new_with_offset(data, 0)
    }

    pub fn new_with_offset(data: &[u8], offset: usize) -> DataReader<'_> {
        DataReader { data, offset }
    }

    pub fn read_u32(&mut self) -> u32 {
        let u = u32::from_le_bytes(
            self.data[self.offset..(self.offset + 4)]
                .try_into()
                .unwrap(),
        );
        self.offset += 4;
        u
    }

    pub fn read_u16(&mut self) -> u16 {
        let u = u16::from_le_bytes(
            self.data[self.offset..(self.offset + 2)]
                .try_into()
                .unwrap(),
        );
        self.offset += 2;
        u
    }
}
