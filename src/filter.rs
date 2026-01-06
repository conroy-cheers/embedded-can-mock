use embedded_can::{ExtendedId, StandardId};

pub enum Filter {
    Standard { id: StandardId, mask: StandardId },
    Extended { id: ExtendedId, mask: ExtendedId },
}

pub enum FilteredStatus {
    Received,
    Filtered,
}

impl From<bool> for FilteredStatus {
    fn from(value: bool) -> Self {
        match value {
            true => FilteredStatus::Received,
            false => FilteredStatus::Filtered,
        }
    }
}

impl Filter {
    pub(crate) fn matches(&self, match_id: embedded_can::Id) -> FilteredStatus {
        match (self, match_id) {
            (Filter::Standard { id, mask }, embedded_can::Id::Standard(standard_id)) => {
                let matches =
                    (standard_id.as_raw() & mask.as_raw()) == (id.as_raw() & mask.as_raw());
                matches.into()
            }
            (Filter::Standard { .. }, embedded_can::Id::Extended(_)) => FilteredStatus::Filtered,
            (Filter::Extended { .. }, embedded_can::Id::Standard(_)) => FilteredStatus::Filtered,
            (Filter::Extended { id, mask }, embedded_can::Id::Extended(extended_id)) => {
                let matches =
                    (extended_id.as_raw() & mask.as_raw()) == (id.as_raw() & mask.as_raw());
                matches.into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_can::{ExtendedId, Id, StandardId};

    #[test]
    fn bool_converts_to_filtered_status() {
        assert!(matches!(FilteredStatus::from(true), FilteredStatus::Received));
        assert!(matches!(FilteredStatus::from(false), FilteredStatus::Filtered));
    }

    #[test]
    fn standard_filter_matches_same_id_when_mask_allows_all_bits() {
        let filter = Filter::Standard {
            id: StandardId::new(0x123).unwrap(),
            mask: StandardId::new(0x7FF).unwrap(),
        };

        let matched = filter.matches(Id::Standard(StandardId::new(0x123).unwrap()));
        let rejected = filter.matches(Id::Standard(StandardId::new(0x321).unwrap()));

        assert!(matches!(matched, FilteredStatus::Received));
        assert!(matches!(rejected, FilteredStatus::Filtered));
    }

    #[test]
    fn standard_filter_respects_masked_bits() {
        let filter = Filter::Standard {
            id: StandardId::new(0x123).unwrap(),
            mask: StandardId::new(0x700).unwrap(),
        };

        let shares_masked_bits = filter.matches(Id::Standard(StandardId::new(0x121).unwrap()));
        let different_masked_bits = filter.matches(Id::Standard(StandardId::new(0x723).unwrap()));

        assert!(matches!(shares_masked_bits, FilteredStatus::Received));
        assert!(matches!(different_masked_bits, FilteredStatus::Filtered));
    }

    #[test]
    fn extended_filter_matches_only_extended_ids() {
        let filter = Filter::Extended {
            id: ExtendedId::new(0x1ABCDE01).unwrap(),
            mask: ExtendedId::new(0x1FFFFFFF).unwrap(),
        };

        let matched = filter.matches(Id::Extended(ExtendedId::new(0x1ABCDE01).unwrap()));
        let rejected_standard = filter.matches(Id::Standard(StandardId::new(0x123).unwrap()));

        assert!(matches!(matched, FilteredStatus::Received));
        assert!(matches!(rejected_standard, FilteredStatus::Filtered));
    }

    #[test]
    fn extended_filter_respects_masked_bits() {
        let filter = Filter::Extended {
            id: ExtendedId::new(0x1ABCDE00).unwrap(),
            mask: ExtendedId::new(0x1FFFFF00).unwrap(),
        };

        let shares_masked_bits =
            filter.matches(Id::Extended(ExtendedId::new(0x1ABCDEF0).unwrap()));
        let different_masked_bits =
            filter.matches(Id::Extended(ExtendedId::new(0x1ABCDD00).unwrap()));

        assert!(matches!(shares_masked_bits, FilteredStatus::Received));
        assert!(matches!(different_masked_bits, FilteredStatus::Filtered));
    }

    #[test]
    fn standard_filter_rejects_extended_ids() {
        let filter = Filter::Standard {
            id: StandardId::new(0x7FF).unwrap(),
            mask: StandardId::new(0x7FF).unwrap(),
        };

        let result = filter.matches(Id::Extended(ExtendedId::new(0x1ABCDE01).unwrap()));

        assert!(matches!(result, FilteredStatus::Filtered));
    }
}
