pub use bus::{BusHandle, InterfaceHandle, MockInterfaceError, TransmitError};
pub use frame::MockFrame;

pub mod bus;
pub mod filter;
pub mod frame;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::Filter;
    use embedded_can::{ExtendedId, Frame as _, Id, StandardId};

    fn standard_frame(id: u16, data: &[u8]) -> MockFrame {
        MockFrame::new(Id::Standard(StandardId::new(id).unwrap()), data).unwrap()
    }

    fn extended_frame(id: u32, data: &[u8]) -> MockFrame {
        MockFrame::new(Id::Extended(ExtendedId::new(id).unwrap()), data).unwrap()
    }

    #[test]
    fn transmit_returns_error_when_not_attached_to_bus() {
        let frame = standard_frame(0x100, &[0x10]);
        let node = InterfaceHandle::new_unattached(vec![]);

        let result = node.transmit(frame);

        assert!(matches!(result, Err(TransmitError::BusNotAttached)));
        assert!(node.received_frames().is_empty());
    }

    #[test]
    fn interface_only_attaches_once() {
        let bus = BusHandle::new();
        let node = InterfaceHandle::new_unattached(vec![]);

        node.attach_to_bus(&bus).unwrap();
        let second_attach = node.attach_to_bus(&bus);

        assert!(matches!(
            second_attach,
            Err(MockInterfaceError::BusAlreadyAttached)
        ));
        assert_eq!(bus.interface_count(), 1);
    }

    #[test]
    fn transmit_delivers_frames_to_all_attached_interfaces() {
        let frame = standard_frame(0x100, &[0x10, 0x20]);

        let bus_ref = BusHandle::new();

        let node1 = bus_ref.add_interface(vec![]).unwrap();
        let node2 = bus_ref.add_interface(vec![]).unwrap();

        node1.transmit(frame.clone()).unwrap();
        node2.transmit(frame.clone()).unwrap();

        assert_eq!(node1.received_frames().len(), 2);
        assert_eq!(node2.received_frames().len(), 2);
        assert_eq!(node1.received_frames()[0], frame);
        assert_eq!(node2.received_frames()[1], frame);
    }

    #[test]
    fn transmit_arc_forwards_frames_via_trait_extension() {
        let frame = extended_frame(0x1ABCDE0, &[0xAB, 0xCD]);
        let bus = BusHandle::new();
        let node = bus.add_interface(vec![]).unwrap();
        node.transmit(frame.clone()).unwrap();

        assert_eq!(node.received_frames(), vec![frame]);
    }

    #[test]
    fn standard_filters_gate_delivery_to_matching_frames() {
        let matching = standard_frame(0x123, &[0xAA]);
        let rejected = standard_frame(0x321, &[0xBB]);

        let bus = BusHandle::new();

        let filtered_node = bus
            .add_interface(vec![Filter::Standard {
                id: StandardId::new(0x123).unwrap(),
                mask: StandardId::new(0x7FF).unwrap(),
            }])
            .unwrap();
        let unfiltered_node = bus.add_interface(vec![]).unwrap();

        unfiltered_node.transmit(matching.clone()).unwrap();
        unfiltered_node.transmit(rejected.clone()).unwrap();

        let filtered_received = filtered_node.received_frames();
        let unfiltered_received = unfiltered_node.received_frames();

        assert_eq!(unfiltered_received.len(), 2);
        assert_eq!(filtered_received, vec![matching.clone()]);
        assert!(!filtered_received.contains(&rejected));
    }

    #[test]
    fn extended_filters_gate_delivery_to_matching_frames() {
        let matching = extended_frame(0x1ABCDE01, &[0xDE, 0xAD]);
        let rejected = extended_frame(0x1ABCD000, &[0xBE, 0xEF]);

        let bus = BusHandle::new();

        let filtered_node = bus
            .add_interface(vec![Filter::Extended {
                id: ExtendedId::new(0x1ABCDE00).unwrap(),
                mask: ExtendedId::new(0x1FFFFF00).unwrap(),
            }])
            .unwrap();
        let unfiltered_node = bus.add_interface(vec![]).unwrap();

        unfiltered_node.transmit(matching.clone()).unwrap();
        unfiltered_node.transmit(rejected.clone()).unwrap();

        let filtered_received = filtered_node.received_frames();
        let unfiltered_received = unfiltered_node.received_frames();

        assert_eq!(unfiltered_received.len(), 2);
        assert_eq!(filtered_received, vec![matching.clone()]);
        assert!(!filtered_received.contains(&rejected));
    }
}
