use crate::error::Jbig2Error;

#[test]
fn error_display() {
    let e = Jbig2Error::InvalidData("bad magic".into());
    assert_eq!(e.to_string(), "invalid data: bad magic");

    let e = Jbig2Error::UnsupportedFeature("color palette".into());
    assert_eq!(e.to_string(), "unsupported feature: color palette");

    let e = Jbig2Error::InternalError("overflow".into());
    assert_eq!(e.to_string(), "internal error: overflow");
}

#[test]
fn error_equality() {
    let a = Jbig2Error::InvalidData("x".into());
    let b = Jbig2Error::InvalidData("x".into());
    let c = Jbig2Error::InvalidData("y".into());
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn error_is_std_error() {
    let e: Box<dyn std::error::Error> = Box::new(Jbig2Error::InternalError("test".into()));
    assert!(e.to_string().contains("test"));
}
