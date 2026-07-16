/// Domain classification for compatibility sea/land mask values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DomainMarker {
    Outside,
    Land,
    Ocean,
    Coast,
}

impl DomainMarker {
    pub fn is_inside(self) -> bool {
        !matches!(self, DomainMarker::Outside)
    }

    pub fn sea_land_mask(self) -> Option<i32> {
        match self {
            DomainMarker::Outside => None,
            DomainMarker::Ocean => Some(0),
            DomainMarker::Land | DomainMarker::Coast => Some(1),
        }
    }

    pub fn from_area_judge_values(is_in_domain: i32, seaorland: i32) -> Self {
        Self::from_area_judge_mask(is_in_domain != 0, seaorland)
    }

    pub fn from_area_judge_mask(is_in_domain: bool, seaorland: i32) -> Self {
        if !is_in_domain {
            DomainMarker::Outside
        } else if seaorland == 0 {
            DomainMarker::Ocean
        } else {
            DomainMarker::Land
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sea_land_mask_does_not_encode_outside() {
        assert_eq!(DomainMarker::Outside.sea_land_mask(), None);
        assert_eq!(DomainMarker::Ocean.sea_land_mask(), Some(0));
        assert_eq!(DomainMarker::Land.sea_land_mask(), Some(1));
        assert_eq!(DomainMarker::Coast.sea_land_mask(), Some(1));
    }
}
