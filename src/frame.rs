//! Mock CAN frame types.
//!
//! [`MockFrame`] implements [`embedded_can::Frame`] and can be used anywhere a `Frame` is
//! expected. This crate stores the payload as an owned `Vec<u8>` for data frames and stores only a
//! DLC for remote frames.

use embedded_can::Frame;

/// Internal representation of frame payload vs remote request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockFrameType {
    /// A data frame with a payload.
    Standard(Vec<u8>),
    /// A remote frame (RTR) that requests `dlc` bytes.
    Remote(usize),
}

/// In-memory CAN frame implementing [`embedded_can::Frame`].
///
/// # Example
///
/// ```
/// use embedded_can::{Frame as _, Id, StandardId};
/// use embedded_can_mock::MockFrame;
///
/// let frame = MockFrame::new(Id::Standard(StandardId::new(0x123).unwrap()), &[0xAA, 0xBB]).unwrap();
/// assert!(!frame.is_remote_frame());
/// assert_eq!(frame.dlc(), 2);
/// assert_eq!(frame.data(), &[0xAA, 0xBB]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockFrame {
    frame_type: MockFrameType,
    id: embedded_can::Id,
}
impl Frame for MockFrame {
    fn new(id: impl Into<embedded_can::Id>, data: &[u8]) -> Option<Self> {
        Some(Self {
            frame_type: MockFrameType::Standard(data.to_vec()),
            id: id.into(),
        })
    }

    fn new_remote(id: impl Into<embedded_can::Id>, dlc: usize) -> Option<Self> {
        Some(Self {
            frame_type: MockFrameType::Remote(dlc),
            id: id.into(),
        })
    }

    fn is_extended(&self) -> bool {
        match self.id {
            embedded_can::Id::Standard(_) => false,
            embedded_can::Id::Extended(_) => true,
        }
    }

    fn is_remote_frame(&self) -> bool {
        match self.frame_type {
            MockFrameType::Standard(_) => false,
            MockFrameType::Remote(_) => true,
        }
    }

    fn id(&self) -> embedded_can::Id {
        self.id
    }

    fn dlc(&self) -> usize {
        match &self.frame_type {
            MockFrameType::Standard(items) => items.len(),
            MockFrameType::Remote(dlc) => *dlc,
        }
    }

    fn data(&self) -> &[u8] {
        match &self.frame_type {
            MockFrameType::Standard(items) => items.as_slice(),
            MockFrameType::Remote(_) => &[],
        }
    }
}
