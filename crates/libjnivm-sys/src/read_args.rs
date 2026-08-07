use crate::types::*;

#[repr(C)]
struct VaList {
    gp_offset: u32,
    fp_offset: u32,
    overflow_arg_area: *mut u8,
    reg_save_area: *mut u8,
}

pub unsafe fn read_va_list_args(args: *mut jvalue) -> (i64, i64, i64, i64) {
    if args.is_null() {
        return (0, 0, 0, 0);
    }
    let vl = &*(args as *const VaList);
    let reg_save = vl.reg_save_area;
    let mut gp = vl.gp_offset as usize;
    let mut overflow = vl.overflow_arg_area;
    if vl.fp_offset != 48 {
        log::warn!("read_va_list_args: FP-ARG CALL gp_offset={} fp_offset={} overflow={:p} reg_save={:p}", vl.gp_offset, vl.fp_offset, vl.overflow_arg_area, vl.reg_save_area);
    }

    let a1 = read_gp_arg(reg_save, &mut gp, &mut overflow);
    let a2 = read_gp_arg(reg_save, &mut gp, &mut overflow);
    let a3 = read_gp_arg(reg_save, &mut gp, &mut overflow);
    let a4 = read_gp_arg(reg_save, &mut gp, &mut overflow);
    (a1, a2, a3, a4)
}

unsafe fn read_gp_arg(reg_save: *mut u8, gp: &mut usize, overflow: &mut *mut u8) -> i64 {
    if *gp < 48 {
        let p = reg_save.add(*gp) as *const i64;
        *gp += 8;
        *p
    } else {
        let p = *overflow as *const i64;
        *overflow = (*overflow).add(8);
        *p
    }
}

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
