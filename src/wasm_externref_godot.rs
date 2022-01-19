use std::io::Write;
use std::ptr::copy_nonoverlapping;

use gdnative::prelude::*;
use wasmtime::{Caller, Extern, ExternRef, Func, Linker, Memory, Trap};

/// Godot module name
pub const GODOT_MODULE: &str = "godot";

#[inline]
pub fn variant_to_externref(object: Variant) -> Option<ExternRef> {
    if object.is_nil() {
        None
    } else {
        Some(ExternRef::new(object))
    }
}

#[inline]
pub fn externref_to_variant(ext: Option<ExternRef>) -> Result<Variant, Trap> {
    match ext {
        None => Ok(Variant::new()),
        Some(v) => match v.data().downcast_ref::<Variant>() {
            None => Err(Trap::new("External reference is not a Godot variant")),
            Some(v) => Ok(v.clone()),
        },
    }
}

#[inline(always)]
pub fn externref_to_variant_nonnull(ext: Option<ExternRef>) -> Result<Variant, Trap> {
    match ext {
        None => Err(Trap::new("Null value")),
        Some(v) => match v.data().downcast_ref::<Variant>() {
            None => Err(Trap::new("External reference is not a Godot variant")),
            Some(v) => Ok(v.clone()),
        },
    }
}

#[inline(always)]
pub fn externref_to_object<T: FromVariant>(ext: Option<ExternRef>) -> Result<T, Trap> {
    match T::from_variant(&externref_to_variant_nonnull(ext)?) {
        Ok(v) => Ok(v),
        Err(e) => Err(Trap::from(Box::new(e) as Box<_>)),
    }
}

macro_rules! variant_convert {
    ($l:ident, $t:ty, ($from:literal, $to:literal)) => {{
        $l.func_wrap(GODOT_MODULE, $from, |v: $t| {
            variant_to_externref(v.to_variant())
        })?;

        $l.func_wrap(GODOT_MODULE, $to, externref_to_object::<$t>)?;
    }};
    ($l:ident, $t:ty => ($($v:ident : $t2:ty),*), ($from:literal, $to:literal)) => {{
        $l.func_wrap(GODOT_MODULE, $from, |$($v: $t2),*| {
            variant_to_externref(<$t as From<($($t2),*)>>::from(($($v),*)).to_variant())
        })?;

        $l.func_wrap(GODOT_MODULE, $to, |v| -> Result<($($t2),*), Trap> {
            Ok(externref_to_object::<$t>(v)?.into())
        })?;
    }};
    ($l:ident, $t:ty => $t2:ty, ($from:literal, $to:literal)) => {{
        $l.func_wrap(GODOT_MODULE, $from, |v: $t2| {
            variant_to_externref(<$t as From<$t2>>::from(v).to_variant())
        })?;

        $l.func_wrap(GODOT_MODULE, $to, |v| -> Result<$t2, Trap> {
            Ok(externref_to_object::<$t>(v)?.into())
        })?;
    }};
    ($l:ident, $o:pat = $t:ty => ($($v:ident $(: $t2:ty)?),+), ($from:literal $ef:expr, $to:literal $et:expr)) => {{
        $l.func_wrap(GODOT_MODULE, $from, |$($v $(: $t2)?),+| {
            variant_to_externref(($ef).to_variant())
        })?;

        $l.func_wrap(GODOT_MODULE, $to, |v| -> Result<_, Trap> {
            let $o = externref_to_object::<$t>(v)?;
            let ($($v,)+) = $et;
            Ok(($($v),+))
        })?;
    }};
}

macro_rules! variant_typecheck {
    ($l:ident, $t:pat, $is:literal) => {
        $l.func_wrap(GODOT_MODULE, $is, |v: Option<ExternRef>| {
            (match v {
                Some(v) => match v.data().downcast_ref::<Variant>() {
                    Some(v) => matches!(v.get_type(), $t),
                    _ => false,
                },
                _ => false,
            }) as i32
        })?
    };
}

macro_rules! object_new {
    ($l:ident, $t:ty, $new:literal) => {
        $l.func_wrap(GODOT_MODULE, $new, || {
            variant_to_externref(<$t>::new().owned_to_variant())
        })?
    };
}

macro_rules! object_call {
    ($l:ident, fn $dup:literal ( $v:ident : $t:ty $(, $a:ident $(: $ta:ty)?)* ) $b:block) => {
        $l.func_wrap(GODOT_MODULE, $dup, |$($a $(: $ta)?,)* $v| {
            let $v = externref_to_object::<$t>($v)?;
            Ok($b)
        })?
    };
    ($l:ident, fn $dup:literal ( $ctx:pat, $v:ident : $t:ty $(, $a:ident $(: $ta:ty)?)* ) $b:block) => {
        $l.func_wrap(GODOT_MODULE, $dup, |$ctx: Caller<_>, $($a $(: $ta)?,)* $v| {
            let $v = externref_to_object::<$t>($v)?;
            Ok($b)
        })?
    };
}

/// Register godot module
pub fn register_godot_externref<T>(linker: &mut Linker<T>) -> anyhow::Result<()> {
    fn get_memory<T>(store: &mut Caller<T>) -> Result<Memory, Trap> {
        match store.get_export("memory") {
            Some(Extern::Memory(m)) => Ok(m),
            _ => Err(Trap::new("No memory exported")),
        }
    }

    linker.func_wrap(
        GODOT_MODULE,
        "print",
        |mut ctx: Caller<_>, s: u32, n: u32| {
            let mem = get_memory(&mut ctx)?.data(&ctx);

            if let Some(s) = mem.get((s as usize)..((s + n) as usize)) {
                godot_print!("{}", String::from_utf8_lossy(s));
                Ok(())
            } else {
                Err(Trap::new("Out of bound"))
            }
        },
    )?;

    linker.func_wrap(
        GODOT_MODULE,
        "warn",
        |mut ctx: Caller<_>, s: u32, n: u32| {
            let mem = get_memory(&mut ctx)?.data(&ctx);

            if let Some(s) = mem.get((s as usize)..((s + n) as usize)) {
                godot_warn!("{}", String::from_utf8_lossy(s));
                Ok(())
            } else {
                Err(Trap::new("Out of bound"))
            }
        },
    )?;

    linker.func_wrap(
        GODOT_MODULE,
        "error",
        |mut ctx: Caller<_>, s: u32, n: u32| -> Result<(), Trap> {
            let mem = get_memory(&mut ctx)?.data(&ctx);

            if let Some(s) = mem.get((s as usize)..((s + n) as usize)) {
                let s = String::from_utf8_lossy(s);
                godot_error!("{}", s);
                Err(Trap::new(s))
            } else {
                Err(Trap::new("Out of bound"))
            }
        },
    )?;

    linker.func_wrap(GODOT_MODULE, "var.is_var", |v: Option<ExternRef>| {
        (match v {
            Some(v) => match v.data().downcast_ref::<Variant>() {
                Some(_) => true,
                None => false,
            },
            _ => false,
        }) as i32
    })?;

    variant_typecheck!(linker, VariantType::I64, "var.is_int");
    variant_typecheck!(linker, VariantType::F64, "var.is_float");
    variant_typecheck!(linker, VariantType::Bool, "var.is_bool");
    variant_typecheck!(linker, VariantType::Vector2, "var.is_vec2");
    variant_typecheck!(linker, VariantType::Vector3, "var.is_vec3");
    variant_typecheck!(linker, VariantType::Quat, "var.is_quat");
    variant_typecheck!(linker, VariantType::Rect2, "var.is_rect2");
    variant_typecheck!(linker, VariantType::Aabb, "var.is_aabb");
    variant_typecheck!(linker, VariantType::Color, "var.is_color");
    variant_typecheck!(linker, VariantType::VariantArray, "var.is_array");
    variant_typecheck!(linker, VariantType::Dictionary, "var.is_dictionary");
    variant_typecheck!(linker, VariantType::GodotString, "var.is_string");
    variant_typecheck!(linker, VariantType::Object, "var.is_object");

    variant_convert!(linker, i32, ("var.from_i32", "var.to_i32"));
    variant_convert!(linker, i64, ("var.from_i64", "var.to_i64"));
    variant_convert!(linker, f32, ("var.from_f32", "var.to_f32"));
    variant_convert!(linker, f64, ("var.from_f64", "var.to_f64"));
    variant_convert!(
        linker,
        v = bool => (v: i32),
        (
            "var.from_bool" (v != 0),
            "var.to_bool" (if v { 1 } else { 0 },)
        )
    );
    variant_convert!(linker, Vector2 => (x: f32, y: f32), ("var.from_vec2", "var.to_vec2"));
    variant_convert!(linker, Vector3 => (x: f32, y: f32, z: f32), ("var.from_vec3", "var.to_vec3"));
    variant_convert!(
        linker,
        Quat { i, j, k, r, .. } = Quat => (r, i, j, k),
        (
            "var.from_quat" Quat::quaternion(i, j, k, r),
            "var.to_quat" (i, j, k, r)
        )
    );
    variant_convert!(
        linker,
        Plane { normal: Vector3 { x, y, z, .. }, d } = Plane => (a, b, c, d),
        (
            "var.from_plane" Plane {
                normal: Vector3::new(a, b, c),
                d,
            },
            "var.to_plane" (x, y, z, d)
        )
    );
    variant_convert!(
        linker,
        Rect2 {
            origin: Point2 { x, y, .. },
            size: Size2 { width, height, .. },
        } = Rect2 => (x, y, w, h),
        (
            "var.from_rect2" Rect2 {
                origin: Point2::new(x, y),
                size: Size2::new(w, h),
            },
            "var.to_rect2" (x, y, width, height)
        )
    );
    variant_convert!(
        linker,
        Aabb {
            position: Vector3 { x, y, z, .. },
            size: Vector3 { x: w, y: h, z: t, .. },
        } = Aabb => (x, y, z, w, h, t),
        (
            "var.from_aabb" Aabb {
                position: Vector3::new(x, y, z),
                size: Vector3::new(w, h, t),
            },
            "var.to_aabb" (x, y, z, w, h, t)
        )
    );
    variant_convert!(
        linker,
        Color { r, g, b, a } = Color => (r, g, b, a),
        (
            "var.from_color" Color { r, g, b, a },
            "var.to_color" (r, g, b, a)
        )
    );

    object_new!(linker, VariantArray<Unique>, "arr.create");
    object_new!(linker, Dictionary<Unique>, "dict.create");
    object_new!(linker, ByteArray, "bytearr.empty");
    object_new!(linker, Int32Array, "intarr.empty");
    object_new!(linker, Float32Array, "floatarr.empty");

    object_call!(linker, fn "arr.duplicate"(v: VariantArray) {
        variant_to_externref(v.duplicate().owned_to_variant())
    });
    object_call!(linker, fn "dict.duplicate"(v: Dictionary) {
        variant_to_externref(v.duplicate().owned_to_variant())
    });

    object_call!(linker, fn "arr.size"(v: VariantArray) {
        v.len()
    });

    object_call!(linker, fn "arr.get"(v: VariantArray, i: i32) {
        if (i < 0) || (i >= v.len()) {
            return Err(Trap::new("Out of bound"));
        } else {
            variant_to_externref(v.get(i))
        }
    });

    object_call!(linker, fn "arr.set"(v: VariantArray, i: i32, x) {
        let x = externref_to_variant(x)?;
        if (i < 0) || (i >= v.len()) {
            return Err(Trap::new("Out of bound"));
        } else {
            v.set(i, x)
        }
    });

    object_call!(linker, fn "arr.grow"(v: VariantArray, x, n: i32) {
        let x = externref_to_variant(x)?;
        let v = unsafe { v.assume_unique() };
        if n > 0 {
            for _ in 0..n {
                v.push(x.clone());
            }
        } else if n < 0 {
            v.resize(v.len() - n);
        }
        v.len()
    });

    object_call!(linker, fn "arr.fill"(v: VariantArray, i: i32, x, n: i32) {
        if (n < 0) || (i < 0) || ((i + n) > v.len()) {
            return Err(Trap::new("Out of bound"));
        }
        let x = externref_to_variant(x)?;
        for j in i..(i + n) {
            v.set(j, x.clone());
        }
    });

    linker.func_wrap(
        GODOT_MODULE,
        "bytearr.create",
        |mut ctx: Caller<_>, s: u32, n: u32| {
            let mem = get_memory(&mut ctx)?.data(&mut ctx);

            if let Some(s) = mem.get((s as usize)..((s + n) as usize)) {
                Ok(variant_to_externref(ByteArray::from_slice(s).to_variant()))
            } else {
                Err(Trap::new("Out of bound"))
            }
        },
    )?;

    object_call!(linker, fn "bytearr.size"(a: ByteArray) {
        a.len()
    });

    object_call!(linker, fn "bytearr.get"(a: ByteArray, i) {
        if (i < 0) || (i >= a.len()) {
            return Err(Trap::new("Out of bound"));
        }
        a.get(i) as i32
    });

    object_call!(linker, fn "bytearr.read"(mut ctx, a: ByteArray, i: u32, s: u32, n: u32) {
        let a = a.read();
        let mem = get_memory(&mut ctx)?.data_mut(&mut ctx);

        if let (Some(d), Some(s)) =
            (
                mem.get_mut((s as usize)..((s + n) as usize)),
                a.get((i as usize)..((i + n) as usize)),
            )
        {
            d.copy_from_slice(s);
        } else {
            return Err(Trap::new("Out of bound"));
        }
    });

    linker.func_wrap(
        GODOT_MODULE,
        "intarr.create",
        |mut ctx: Caller<_>, s: u32, n: u32| {
            let mem = get_memory(&mut ctx)?.data(&mut ctx);

            if let Some(s) = mem.get((s as usize)..((s + n * 4) as usize)) {
                let mut d = Int32Array::new();
                d.resize(n as i32);
                {
                    let d = &mut d.write();
                    let pd = d.as_mut_ptr() as *mut u8;
                    let ps = s.as_ptr();
                    unsafe { copy_nonoverlapping(ps, pd, (n * 4) as usize) };
                }
                Ok(variant_to_externref(d.owned_to_variant()))
            } else {
                Err(Trap::new("Out of bound"))
            }
        },
    )?;

    object_call!(linker, fn "intarr.size"(a: Int32Array) {
        a.len()
    });

    object_call!(linker, fn "intarr.get"(a: Int32Array, i) {
        if (i < 0) || (i >= a.len()) {
            return Err(Trap::new("Out of bound"));
        }
        a.get(i)
    });

    object_call!(linker, fn "intarr.read"(mut ctx, a: Int32Array, i: u32, s: u32, n: u32) {
        let a = a.read();
        let mem = get_memory(&mut ctx)?.data_mut(&mut ctx);

        if let (Some(d), Some(s)) =
            (
                mem.get_mut((s as usize)..((s + n * 4) as usize)),
                a.get((i as usize)..((i + n) as usize)),
            )
        {
            let pd = d.as_mut_ptr();
            let ps = s.as_ptr() as *const u8;
            unsafe { copy_nonoverlapping(ps, pd, (n * 4) as usize) };
        } else {
            return Err(Trap::new("Out of bound"));
        }
    });

    linker.func_wrap(
        GODOT_MODULE,
        "floatarr.create",
        |mut ctx: Caller<_>, s: u32, n: u32| {
            let mem = get_memory(&mut ctx)?.data(&mut ctx);

            if let Some(s) = mem.get((s as usize)..((s + n * 4) as usize)) {
                let mut d = Float32Array::new();
                d.resize(n as i32);
                {
                    let d = &mut d.write();
                    let pd = d.as_mut_ptr() as *mut u8;
                    let ps = s.as_ptr();
                    unsafe { copy_nonoverlapping(ps, pd, (n * 4) as usize) };
                }
                Ok(variant_to_externref(d.owned_to_variant()))
            } else {
                Err(Trap::new("Out of bound"))
            }
        },
    )?;

    object_call!(linker, fn "floatarr.size"(a: Float32Array) {
        a.len()
    });

    object_call!(linker, fn "floatarr.get"(a: Float32Array, i) {
        if (i < 0) || (i >= a.len()) {
            return Err(Trap::new("Out of bound"));
        }
        a.get(i)
    });

    object_call!(linker, fn "floatarr.read"(mut ctx, a: Float32Array, i: u32, s: u32, n: u32) {
        let a = a.read();
        let mem = get_memory(&mut ctx)?.data_mut(&mut ctx);

        if let (Some(d), Some(s)) =
            (
                mem.get_mut((s as usize)..((s + n * 4) as usize)),
                a.get((i as usize)..((i + n) as usize)),
            )
        {
            let pd = d.as_mut_ptr();
            let ps = s.as_ptr() as *const u8;
            unsafe { copy_nonoverlapping(ps, pd, (n * 4) as usize) };
        } else {
            return Err(Trap::new("Out of bound"));
        }
    });

    object_call!(linker, fn "dict.size"(d: Dictionary) {
        d.len()
    });

    object_call!(linker, fn "dict.key_in"(d: Dictionary, k) {
        d.contains(externref_to_variant(k)?) as i32
    });

    object_call!(linker, fn "dict.get"(d: Dictionary, k) {
        variant_to_externref(d.get(externref_to_variant(k)?))
    });

    object_call!(linker, fn "dict.set"(d: Dictionary, k, v) {
        d.update(externref_to_variant(k)?, externref_to_variant(v)?);
    });

    object_call!(linker, fn "dict.delete"(d: Dictionary, k) {
        unsafe { d.assume_unique() }.erase(externref_to_variant(k)?);
    });

    object_call!(linker, fn "dict.clear"(d: Dictionary) {
        unsafe { d.assume_unique() }.clear();
    });

    object_call!(linker, fn "dict.iter"(mut ctx, d: Dictionary, f: Option<Func>) {
        let f = match f {
            None => return Err(Trap::new("Function is null")),
            Some(f) => match f.typed::<(Option<ExternRef>, Option<ExternRef>), i32, _>(&ctx) {
                Ok(f) => f,
                Err(e) => return Err(Trap::from(e)),
            },
        };
        for (k, v) in d.iter() {
            if f.call(&mut ctx, (variant_to_externref(k), variant_to_externref(v)))?
                != 0
            {
                break;
            }
        }
    });

    linker.func_wrap(
        GODOT_MODULE,
        "str.create",
        |mut ctx: Caller<_>, s: u32, n: u32| {
            let mem = get_memory(&mut ctx)?.data(&ctx);

            if let Some(s) = mem.get((s as usize)..((s + n) as usize)) {
                Ok(variant_to_externref(
                    GodotString::from_str(String::from_utf8_lossy(s)).to_variant(),
                ))
            } else {
                Err(Trap::new("Out of bound"))
            }
        },
    )?;

    object_call!(linker, fn "str.read"(mut ctx, v: GodotString, s: u32, n: u32) {
        let mem = get_memory(&mut ctx)?.data_mut(&mut ctx);

        if let Some(s) = mem.get_mut((s as usize)..((s + n) as usize)) {
            if let Err(e) = write!(&mut *s, "{}", v) {
                return Err(Trap::from(anyhow::Error::new(e)));
            }
        } else {
            return Err(Trap::new("Out of bound"));
        }
    });

    object_call!(linker, fn "str.size"(s: GodotString) {
        s.len() as u32
    });

    object_call!(linker, fn "str.is_valid_float"(s: GodotString) {
        s.is_valid_float() as i32
    });

    object_call!(linker, fn "str.is_valid_integer"(s: GodotString) {
        s.is_valid_integer() as i32
    });

    object_call!(linker, fn "str.is_valid_hex_number"(s: GodotString, p: i32) {
        s.is_valid_hex_number(p != 0) as i32
    });

    object_call!(linker, fn "str.to_i32"(s: GodotString) {
        s.to_i32()
    });

    object_call!(linker, fn "str.to_f32"(s: GodotString) {
        s.to_f32()
    });

    object_call!(linker, fn "str.to_f64"(s: GodotString) {
        s.to_f64()
    });

    object_call!(linker, fn "str.hex_to_int"(s: GodotString) {
        s.hex_to_int()
    });

    object_call!(linker, fn "str.to_lower"(s: GodotString) {
        variant_to_externref(s.to_lowercase().to_variant())
    });

    object_call!(linker, fn "str.to_upper"(s: GodotString) {
        variant_to_externref(s.to_uppercase().to_variant())
    });

    object_call!(linker, fn "str.capitalize"(s: GodotString) {
        variant_to_externref(s.capitalize().to_variant())
    });

    object_call!(linker, fn "str.c_escape"(s: GodotString) {
        variant_to_externref(s.c_escape().to_variant())
    });

    object_call!(linker, fn "str.c_unescape"(s: GodotString) {
        variant_to_externref(s.c_unescape().to_variant())
    });

    object_call!(linker, fn "str.http_escape"(s: GodotString) {
        variant_to_externref(s.http_escape().to_variant())
    });

    object_call!(linker, fn "str.http_unescape"(s: GodotString) {
        variant_to_externref(s.http_unescape().to_variant())
    });

    object_call!(linker, fn "str.xml_escape"(s: GodotString) {
        variant_to_externref(s.xml_escape().to_variant())
    });

    object_call!(linker, fn "str.xml_escape_with_quotes"(s: GodotString) {
        variant_to_externref(s.xml_escape_with_quotes().to_variant())
    });

    object_call!(linker, fn "str.xml_unescape"(s: GodotString) {
        variant_to_externref(s.xml_unescape().to_variant())
    });

    object_call!(linker, fn "str.percent_encode"(s: GodotString) {
        variant_to_externref(s.percent_encode().to_variant())
    });

    object_call!(linker, fn "str.percent_decode"(s: GodotString) {
        variant_to_externref(s.percent_decode().to_variant())
    });

    object_call!(linker, fn "str.begins_with"(s: GodotString, o) {
        s.begins_with(&externref_to_object(o)?) as i32
    });

    object_call!(linker, fn "str.ends_with"(s: GodotString, o) {
        s.ends_with(&externref_to_object(o)?) as i32
    });

    object_call!(linker, fn "color.h"(c: Color) {
        c.h()
    });

    object_call!(linker, fn "color.s"(c: Color) {
        c.s()
    });

    object_call!(linker, fn "color.v"(c: Color) {
        c.v()
    });

    object_call!(linker, fn "color.lerp"(c: Color, o, w: f32) {
        variant_to_externref(c.lerp(externref_to_object(o)?, w).to_variant())
    });

    object_call!(linker, fn "object.callv"(o: Ref<Object, Shared>, args, name) {
        let name: GodotString = externref_to_object(name)?;
        variant_to_externref(unsafe {
            o.assume_safe().callv(name, externref_to_object(args)?)
        })
    });

    object_call!(linker, fn "object.callv_deferred"(o: Ref<Object, Shared>, args, name) {
        let name: GodotString = externref_to_object(name)?;
        let args: Vec<_> = externref_to_object::<VariantArray>(args)?.iter().collect();
        unsafe {
            o.assume_safe().call_deferred(name, &args);
        }
    });

    Ok(())
}
