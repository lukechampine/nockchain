#[derive(Copy, Clone, Debug)]
pub enum IpAddressCheckDecision {
    Allow,
    Block(IpAddressCheckReason),
    Review(IpAddressCheckReason),
}

impl IpAddressCheckDecision {
    pub fn decision_as_str(&self) -> &str {
        match self {
            IpAddressCheckDecision::Allow => "ALLOW",
            IpAddressCheckDecision::Block(_) => "BLOCK",
            IpAddressCheckDecision::Review(_) => "REVIEW",
        }
    }

    pub fn reason_as_str(&self) -> Option<&str> {
        match self {
            IpAddressCheckDecision::Allow => None,
            IpAddressCheckDecision::Block(reason) | IpAddressCheckDecision::Review(reason) => {
                Some(match reason {
                    IpAddressCheckReason::Country => "COUNTRY",
                    IpAddressCheckReason::Vpn => "VPN",
                })
            }
        }
    }

    pub fn from_str(decision: &str, reason: Option<&str>) -> Option<Self> {
        match decision.to_uppercase().as_str() {
            "ALLOW" => Some(IpAddressCheckDecision::Allow),
            "BLOCK" => {
                let reason = Self::parse_reason(reason)?;
                Some(IpAddressCheckDecision::Block(reason))
            }
            "REVIEW" => {
                let reason = Self::parse_reason(reason)?;
                Some(IpAddressCheckDecision::Review(reason))
            }
            _ => None,
        }
    }

    fn parse_reason(reason: Option<&str>) -> Option<IpAddressCheckReason> {
        match reason?.to_uppercase().as_str() {
            "COUNTRY" => Some(IpAddressCheckReason::Country),
            "VPN" => Some(IpAddressCheckReason::Vpn),
            _ => None,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum IpAddressCheckReason {
    Country,
    Vpn,
}
