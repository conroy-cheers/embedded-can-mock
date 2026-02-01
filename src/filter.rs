//! Acceptance filter validation for the mock bus.
//!
//! Filters are expressed as [`embedded_can_interface::IdMaskFilter`] values. The mock validates
//! that the `id` and `mask` are of compatible kinds (standard vs extended). Mismatched kinds are
//! rejected because they cannot sensibly match any incoming ID.

use embedded_can::Id;
use embedded_can_interface::{IdMask, IdMaskFilter};

/// Errors returned when validating acceptance filters.
#[derive(Debug)]
pub enum FilterError {
    /// The filter’s `id` kind does not match its `mask` kind (standard vs extended).
    KindMismatch,
}

pub(crate) fn matches(filter: &IdMaskFilter, match_id: Id) -> bool {
    match (filter.id, filter.mask, match_id) {
        (embedded_can_interface::Id::Standard(fid), IdMask::Standard(mask), Id::Standard(id)) => {
            (id.as_raw() & mask) == (fid.as_raw() & mask)
        }
        (embedded_can_interface::Id::Extended(fid), IdMask::Extended(mask), Id::Extended(id)) => {
            (id.as_raw() & mask) == (fid.as_raw() & mask)
        }
        _ => false,
    }
}

pub(crate) fn validate_filter(filter: &IdMaskFilter) -> Result<(), FilterError> {
    match (filter.id, filter.mask) {
        (embedded_can_interface::Id::Standard(_), IdMask::Standard(_)) => Ok(()),
        (embedded_can_interface::Id::Extended(_), IdMask::Extended(_)) => Ok(()),
        _ => Err(FilterError::KindMismatch),
    }
}

pub(crate) fn validate_filters(filters: &[IdMaskFilter]) -> Result<(), FilterError> {
    for f in filters {
        validate_filter(f)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_can::{ExtendedId, StandardId};

    #[test]
    fn standard_filter_matches() {
        let filter = IdMaskFilter {
            id: embedded_can_interface::Id::Standard(StandardId::new(0x123).unwrap()),
            mask: IdMask::Standard(0x7FF),
        };
        let matched = matches(&filter, Id::Standard(StandardId::new(0x123).unwrap()));
        let rejected = matches(&filter, Id::Standard(StandardId::new(0x321).unwrap()));
        assert!(matched);
        assert!(!rejected);
    }

    #[test]
    fn extended_filter_matches() {
        let filter = IdMaskFilter {
            id: embedded_can_interface::Id::Extended(ExtendedId::new(0x1ABCDE00).unwrap()),
            mask: IdMask::Extended(0x1FFFFF00),
        };
        let matched = matches(&filter, Id::Extended(ExtendedId::new(0x1ABCDE01).unwrap()));
        let rejected = matches(&filter, Id::Extended(ExtendedId::new(0x1ABCDD00).unwrap()));
        assert!(matched);
        assert!(!rejected);
    }

    #[test]
    fn mismatched_kinds_reject() {
        let filter = IdMaskFilter {
            id: embedded_can_interface::Id::Standard(StandardId::new(0x7FF).unwrap()),
            mask: IdMask::Standard(0x7FF),
        };
        assert!(!matches(
            &filter,
            Id::Extended(ExtendedId::new(0x1ABCDE01).unwrap())
        ));
    }
}
