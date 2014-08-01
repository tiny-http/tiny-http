use std::io::IoResult;

pub enum Frame {
    /// id, "end_stream" flag, data reader, data length
    Data(StreamIdentifier, bool, Box<Reader+Send>, uint),

    // 
    Headers,

    // 
    Priority,

    // 
    RstStream(ErrorCode),

    // setting and value
    Settings(SettingID, u32),

    //
    PushPromise,

    // Ack flag, and payload
    Ping(bool, u64),

    //
    GoAway,

    //
    WindowUpdate(u32),

    //
    Continuation,

    /// An Invalid frame.
    Invalid,
}

pub struct StreamIdentifier(u32);

/// Represents an error code in HTTP 2.
#[deriving(FromPrimitive, Show, Clone, PartialEq, Eq)]
pub enum ErrorCode {
    NoError = 0x0,
    ProtocolError = 0x1,
    InternalError = 0x2,
    FlowControlError = 0x3,
    SettingsTimeout = 0x4,
    StreamClosed = 0x5,
    FrameSizeError = 0x6,
    RefusedStream = 0x7,
    Cancel = 0x8,
    CompressionError = 0x9,
    ConnectError = 0xa,
    EnhanceYourCalm = 0xb,
    InadequateSecurity = 0xc,
}

// A setting in HTTP 2.
#[deriving(FromPrimitive, Show, Clone, PartialEq, Eq)]
pub enum SettingID {
    HeaderTableSize = 0x1,
    EnablePush = 0x2,
    MaxConcurrentStreams = 0x3,
    InitialWindowSize = 0x4,
    MaxFrameSize = 0x5,
    MaxHeaderListSize = 0x6,
}

pub struct Parser<R> {
    reader: R,
}

#[packed]
struct Header {
    length: [u8, ..3],
    frame_type: u8,
    flags: u8,
    identifier: u32,        // 
}

impl<R: Reader> Parser<R> {
    pub fn new(reader: R) -> Parser<R> {
        Parser {
            reader: reader,
        }
    }
}

impl<R: Reader> Iterator<Frame> for Parser<R> {
    fn next(&mut self) -> Option<Frame> {
        unimplemented!()
    }
}

impl Frame {
    pub fn write<W: Writer>(self, writer: &mut W) -> IoResult<()> {
        match self {
            Ping(ack, payload) => {
                try!(write_header(writer, 8, 0x6, if ack { 0x1 } else { 0x0 }, 0x0));
                try!(writer.write_be_u64(payload));
            },

            Invalid => fail!("Can't write an invalid frame"),
            _ => unimplemented!()
        }

        Ok(())
    }
}

fn write_header<W: Writer>(writer: &mut W, length: u32, frame_type: u8, flags: u8, identifier: u32) -> IoResult<()> {
    assert!(length < (1 << 24));
    assert!((identifier & 0x80000000) == 0);

    let length0 = (length >> 16) as u8;
    let length1 = (length >> 8) as u8;
    let length2 = length as u8;

    try!(writer.write_u8(length0));
    try!(writer.write_u8(length1));
    try!(writer.write_u8(length2));

    try!(writer.write_u8(frame_type));
    try!(writer.write_u8(flags));

    try!(writer.write_be_u32(identifier));

    Ok(())
}
