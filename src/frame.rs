use embedded_can::Frame;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockFrameType {
    Standard(Vec<u8>),
    Remote(usize),
}

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
