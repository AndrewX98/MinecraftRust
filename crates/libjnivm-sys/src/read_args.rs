use crate::types::*;

pub unsafe fn read_jvalue_args(args: *mut jvalue) -> (i64, i64, i64, i64) {
    if args.is_null() {
        return (0, 0, 0, 0);
    }
    let a = &*args;
    let v1 = std::mem::transmute::<jvalue, i64>(*a);
    let v2 = std::mem::transmute::<jvalue, i64>(*args.offset(1));
    let v3 = std::mem::transmute::<jvalue, i64>(*args.offset(2));
    let v4 = std::mem::transmute::<jvalue, i64>(*args.offset(3));
    (v1, v2, v3, v4)
}
