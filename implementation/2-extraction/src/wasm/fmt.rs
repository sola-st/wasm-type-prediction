use std::fmt;

use wasmparser::{MemoryImmediate, Operator, Type, V128};

pub fn type_str(ty: Type) -> &'static str {
    use wasmparser::Type::*;
    match ty {
        I32 => "i32",
        I64 => "i64",
        F32 => "f32",
        F64 => "f64",
        V128 => "v128",
        FuncRef => "funcref",
        ExternRef => "externref",
        ExnRef => "exnref",
        Func => unimplemented!(),
        EmptyBlockType => unimplemented!(),
    }
}

pub fn fmt_instr(fmt: &mut impl fmt::Write, op: &Operator, param_local_idx: Option<u32>) -> fmt::Result {
    // Print instruction mnemonic.
    fmt.write_str(instr_name(op))?;

    // Print instruction arguments, or <param> for the local we currently extract the type for.
    use wasmparser::Operator::*;
    #[rustfmt::skip]
    match op {
        LocalGet { local_index } 
        | LocalSet { local_index } 
        | LocalTee { local_index } if Some(*local_index) == param_local_idx => fmt.write_str(" <param>")?,
        _ => fmt_instr_args(fmt, op)?
    };

    // TODO print types of globals, calls, etc.

    Ok(())
}

#[rustfmt::skip]
fn fmt_instr_args(fmt: &mut impl fmt::Write, op: &Operator) -> fmt::Result {
    // Contains all instructions with arguments below, 
    // but those which we do not print are commented out.
    use wasmparser::Operator::*;
    match op {
        // Block { ty } => "block",
        // Loop { ty } => "loop",
        // If { ty } => "if",

        // Try { ty } => "try",
        // Catch { index } => "catch",
        // Throw { index } => "throw",

        Rethrow { relative_depth }
        | Br { relative_depth }
        | BrIf { relative_depth } => write!(fmt, " {}", relative_depth),
        
        BrTable { table } => {
            for result in table.targets() {
                let (relative_depth, _default_label) = result.unwrap();
                write!(fmt, " {}", relative_depth)?;
            }
            Ok(())
        },

        // Call { function_index } => "call",
        // CallIndirect { table_index, index } => "call_indirect",

        // ReturnCall { function_index } => "return_call",
        // ReturnCallIndirect { table_index, index } => "return_call_indirect",

        // Delegate { relative_depth } => "delegate",

        // TypedSelect { ty } => "select",

        // RefNull { ty } => "ref.null",
        // RefFunc { function_index } => "ref.func",

        LocalGet { local_index } 
        | LocalSet { local_index } 
        | LocalTee { local_index } => write!(fmt, " {}", local_index),

        GlobalGet { global_index }
        | GlobalSet { global_index } => write!(fmt, " {}", global_index),

        MemorySize { mem: 0, .. }
        | MemoryGrow { mem: 0, .. } =>  Ok(()),

        MemorySize { mem, .. }
        | MemoryGrow { mem, .. } => write!(fmt, " {}", mem),

        I32Const { value } => write!(fmt, " {}", value),
        I64Const { value } => write!(fmt, " {}", value),
        F32Const { value } => write!(fmt, " {}", f32::from_bits(value.bits())),
        F64Const { value } => write!(fmt, " {}", f64::from_bits(value.bits())),

        MemoryInit { segment, mem } => write!(fmt, " {} {}", segment, mem),
        DataDrop { segment } => write!(fmt, " {}", segment),
        MemoryCopy { src, dst } => write!(fmt, " {} {}", src, dst),
        MemoryFill { mem } => write!(fmt, " {}", mem),

        TableInit { table, segment } => write!(fmt, " {} {}", table, segment),
        ElemDrop { segment } => write!(fmt, " {}", segment),
        TableCopy { dst_table, src_table } => write!(fmt, " {} {}", src_table, dst_table),
        TableGet { table }
        | TableSet { table }
        | TableGrow { table }
        | TableSize { table }
        | TableFill { table } => write!(fmt, " {}", table),

        I32Load { memarg }
        | I64Load { memarg }
        | F32Load { memarg }
        | F64Load { memarg }
        | I32Load8S { memarg }
        | I32Load8U { memarg }
        | I32Load16S { memarg }
        | I32Load16U { memarg }
        | I64Load8S { memarg }
        | I64Load8U { memarg }
        | I64Load16S { memarg }
        | I64Load16U { memarg }
        | I64Load32S { memarg }
        | I64Load32U { memarg }

        | I32Store { memarg }
        | I64Store { memarg }
        | F32Store { memarg }
        | F64Store { memarg }
        | I32Store8 { memarg }
        | I32Store16 { memarg }
        | I64Store8 { memarg }
        | I64Store16 { memarg }
        | I64Store32 { memarg }

        | MemoryAtomicNotify { memarg }
        | MemoryAtomicWait32 { memarg }
        | MemoryAtomicWait64 { memarg }

        | I32AtomicLoad { memarg }
        | I64AtomicLoad { memarg }
        | I32AtomicLoad8U { memarg }
        | I32AtomicLoad16U { memarg }
        | I64AtomicLoad8U { memarg }
        | I64AtomicLoad16U { memarg }
        | I64AtomicLoad32U { memarg }

        | I32AtomicStore { memarg }
        | I64AtomicStore { memarg }
        | I32AtomicStore8 { memarg }
        | I32AtomicStore16 { memarg }
        | I64AtomicStore8 { memarg }
        | I64AtomicStore16 { memarg }
        | I64AtomicStore32 { memarg }

        | I32AtomicRmwAdd { memarg }
        | I64AtomicRmwAdd { memarg }
        | I32AtomicRmw8AddU { memarg }
        | I32AtomicRmw16AddU { memarg }
        | I64AtomicRmw8AddU { memarg }
        | I64AtomicRmw16AddU { memarg }
        | I64AtomicRmw32AddU { memarg }

        | I32AtomicRmwSub { memarg }
        | I64AtomicRmwSub { memarg }
        | I32AtomicRmw8SubU { memarg }
        | I32AtomicRmw16SubU { memarg }
        | I64AtomicRmw8SubU { memarg }
        | I64AtomicRmw16SubU { memarg }
        | I64AtomicRmw32SubU { memarg }

        | I32AtomicRmwAnd { memarg }
        | I64AtomicRmwAnd { memarg }
        | I32AtomicRmw8AndU { memarg }
        | I32AtomicRmw16AndU { memarg }
        | I64AtomicRmw8AndU { memarg }
        | I64AtomicRmw16AndU { memarg }
        | I64AtomicRmw32AndU { memarg }

        | I32AtomicRmwOr { memarg }
        | I64AtomicRmwOr { memarg }
        | I32AtomicRmw8OrU { memarg }
        | I32AtomicRmw16OrU { memarg }
        | I64AtomicRmw8OrU { memarg }
        | I64AtomicRmw16OrU { memarg }
        | I64AtomicRmw32OrU { memarg }

        | I32AtomicRmwXor { memarg }
        | I64AtomicRmwXor { memarg }
        | I32AtomicRmw8XorU { memarg }
        | I32AtomicRmw16XorU { memarg }
        | I64AtomicRmw8XorU { memarg }
        | I64AtomicRmw16XorU { memarg }
        | I64AtomicRmw32XorU { memarg }

        | I32AtomicRmwXchg { memarg }
        | I64AtomicRmwXchg { memarg }
        | I32AtomicRmw8XchgU { memarg }
        | I32AtomicRmw16XchgU { memarg }
        | I64AtomicRmw8XchgU { memarg }
        | I64AtomicRmw16XchgU { memarg }
        | I64AtomicRmw32XchgU { memarg }

        | I32AtomicRmwCmpxchg { memarg }
        | I64AtomicRmwCmpxchg { memarg }
        | I32AtomicRmw8CmpxchgU { memarg }
        | I32AtomicRmw16CmpxchgU { memarg }
        | I64AtomicRmw8CmpxchgU { memarg }
        | I64AtomicRmw16CmpxchgU { memarg }
        | I64AtomicRmw32CmpxchgU { memarg }

        | V128Load { memarg }
        | V128Store { memarg }

        | V128Load8Splat { memarg }
        | V128Load16Splat { memarg }
        | V128Load32Splat { memarg }
        | V128Load64Splat { memarg }

        | V128Load32Zero { memarg }
        | V128Load64Zero { memarg }

        | V128Load8x8S { memarg }
        | V128Load8x8U { memarg }
        | V128Load16x4S { memarg }
        | V128Load16x4U { memarg }
        | V128Load32x2S { memarg }
        | V128Load32x2U { memarg } => fmt_memarg(fmt, memarg),

        V128Const { value } => fmt_v128(fmt, value),

        I8x16ExtractLaneS { lane }
        | I8x16ExtractLaneU { lane }
        | I8x16ReplaceLane { lane }
        | I16x8ExtractLaneS { lane }
        | I16x8ExtractLaneU { lane }
        | I16x8ReplaceLane { lane }
        | I32x4ExtractLane { lane }
        | I32x4ReplaceLane { lane }
        | I64x2ExtractLane { lane }
        | I64x2ReplaceLane { lane }
        | F32x4ExtractLane { lane }
        | F32x4ReplaceLane { lane }
        | F64x2ExtractLane { lane }
        | F64x2ReplaceLane { lane } => write!(fmt, " {}", lane),
        
        I8x16Shuffle { lanes } => {
            for lane in lanes {
                write!(fmt, " {}", lane)?;
            }
            Ok(())
        },

        V128Load8Lane { memarg, lane }
        | V128Load16Lane { memarg, lane }
        | V128Load32Lane { memarg, lane }
        | V128Load64Lane { memarg, lane }

        | V128Store8Lane { memarg, lane } 
        | V128Store16Lane { memarg, lane } 
        | V128Store32Lane { memarg, lane } 
        | V128Store64Lane { memarg, lane } => {
            fmt_memarg(fmt, memarg)?;
            write!(fmt, " {}", lane)
        }

        _ => Ok(())
    }
}

// see https://github.com/bytecodealliance/wasm-tools/blob/main/crates/wasmprinter/src/lib.rs mem_instr
fn fmt_memarg(fmt: &mut impl fmt::Write, memarg: &MemoryImmediate) -> fmt::Result {
    if memarg.memory != 0 {
        write!(fmt, " (memory {})", memarg.memory)?;
    }
    if memarg.offset != 0 {
        write!(fmt, " offset={}", memarg.offset)?;
    }
    // TODO Print alignment as well?
    // For that we would need the default_alignment, which depends on the instruction.
    Ok(())
}

// see https://github.com/bytecodealliance/wasm-tools/blob/main/crates/wasmprinter/src/lib.rs print_operator
fn fmt_v128(fmt: &mut impl fmt::Write, value: &V128) -> fmt::Result {
    for chunk in value.bytes().chunks(4) {
        write!(
            fmt,
            " 0x{:02x}{:02x}{:02x}{:02x}",
            chunk[3], chunk[2], chunk[1], chunk[0]
        )?;
    }
    Ok(())
}

pub fn instr_name(op: &Operator) -> &'static str {
    use wasmparser::Operator::*;
    #[allow(unused_variables)]
    match op {
        Nop => "nop",
        Unreachable => "unreachable",

        Block { ty } => "block",
        Loop { ty } => "loop",
        If { ty } => "if",
        Else => "else",

        Try { ty } => "try",
        Catch { index } => "catch",
        Throw { index } => "throw",
        Rethrow { relative_depth } => "rethrow",
        Unwind => "unwind",

        End => "end",
        Br { relative_depth } => "br",
        BrIf { relative_depth } => "br_if",
        BrTable { table } => "br_table",

        Return => "return",
        Call { function_index } => "call",
        CallIndirect { table_index, index } => "call_indirect",

        ReturnCall { function_index } => "return_call",
        ReturnCallIndirect { table_index, index } => "return_call_indirect",

        Delegate { relative_depth } => "delegate",
        CatchAll => "catch_all",

        Drop => "drop",
        Select => "select",
        TypedSelect { ty } => "select",

        LocalGet { local_index } => "local.get",
        LocalSet { local_index } => "local.set",
        LocalTee { local_index } => "local.tee",

        GlobalGet { global_index } => "global.get",
        GlobalSet { global_index } => "global.set",

        I32Load { memarg } => "i32.load",
        I64Load { memarg } => "i64.load",
        F32Load { memarg } => "f32.load",
        F64Load { memarg } => "f64.load",
        I32Load8S { memarg } => "i32.load8_s",
        I32Load8U { memarg } => "i32.load8_u",
        I32Load16S { memarg } => "i32.load16_s",
        I32Load16U { memarg } => "i32.load16_u",
        I64Load8S { memarg } => "i64.load8_s",
        I64Load8U { memarg } => "i64.load8_u",
        I64Load16S { memarg } => "i64.load16_s",
        I64Load16U { memarg } => "i64.load16_u",
        I64Load32S { memarg } => "i64.load32_s",
        I64Load32U { memarg } => "i64.load32_u",

        I32Store { memarg } => "i32.store",
        I64Store { memarg } => "i64.store",
        F32Store { memarg } => "f32.store",
        F64Store { memarg } => "f64.store",
        I32Store8 { memarg } => "i32.store8",
        I32Store16 { memarg } => "i32.store16",
        I64Store8 { memarg } => "i64.store8",
        I64Store16 { memarg } => "i64.store16",
        I64Store32 { memarg } => "i64.store32",

        MemorySize { mem, .. } => "memory.size",
        MemoryGrow { mem, .. } => "memory.grow",

        I32Const { value } => "i32.const",
        I64Const { value } => "i64.const",
        F32Const { value } => "f32.const",
        F64Const { value } => "f64.const",

        RefNull { ty } => "ref.null",
        RefIsNull => "ref.is_null",
        RefFunc { function_index } => "ref.func",

        I32Eqz => "i32.eqz",
        I32Eq => "i32.eq",
        I32Ne => "i32.ne",
        I32LtS => "i32.lt_s",
        I32LtU => "i32.lt_u",
        I32GtS => "i32.gt_s",
        I32GtU => "i32.gt_u",
        I32LeS => "i32.le_s",
        I32LeU => "i32.le_u",
        I32GeS => "i32.ge_s",
        I32GeU => "i32.ge_u",

        I64Eqz => "i64.eqz",
        I64Eq => "i64.eq",
        I64Ne => "i64.ne",
        I64LtS => "i64.lt_s",
        I64LtU => "i64.lt_u",
        I64GtS => "i64.gt_s",
        I64GtU => "i64.gt_u",
        I64LeS => "i64.le_s",
        I64LeU => "i64.le_u",
        I64GeS => "i64.ge_s",
        I64GeU => "i64.ge_u",

        F32Eq => "f32.eq",
        F32Ne => "f32.ne",
        F32Lt => "f32.lt",
        F32Gt => "f32.gt",
        F32Le => "f32.le",
        F32Ge => "f32.ge",

        F64Eq => "f64.eq",
        F64Ne => "f64.ne",
        F64Lt => "f64.lt",
        F64Gt => "f64.gt",
        F64Le => "f64.le",
        F64Ge => "f64.ge",

        I32Clz => "i32.clz",
        I32Ctz => "i32.ctz",
        I32Popcnt => "i32.popcnt",
        I32Add => "i32.add",
        I32Sub => "i32.sub",
        I32Mul => "i32.mul",
        I32DivS => "i32.div_s",
        I32DivU => "i32.div_u",
        I32RemS => "i32.rem_s",
        I32RemU => "i32.rem_u",
        I32And => "i32.and",
        I32Or => "i32.or",
        I32Xor => "i32.xor",
        I32Shl => "i32.shl",
        I32ShrS => "i32.shr_s",
        I32ShrU => "i32.shr_u",
        I32Rotl => "i32.rotl",
        I32Rotr => "i32.rotr",

        I64Clz => "i64.clz",
        I64Ctz => "i64.ctz",
        I64Popcnt => "i64.popcnt",
        I64Add => "i64.add",
        I64Sub => "i64.sub",
        I64Mul => "i64.mul",
        I64DivS => "i64.div_s",
        I64DivU => "i64.div_u",
        I64RemS => "i64.rem_s",
        I64RemU => "i64.rem_u",
        I64And => "i64.and",
        I64Or => "i64.or",
        I64Xor => "i64.xor",
        I64Shl => "i64.shl",
        I64ShrS => "i64.shr_s",
        I64ShrU => "i64.shr_u",
        I64Rotl => "i64.rotl",
        I64Rotr => "i64.rotr",

        F32Abs => "f32.abs",
        F32Neg => "f32.neg",
        F32Ceil => "f32.ceil",
        F32Floor => "f32.floor",
        F32Trunc => "f32.trunc",
        F32Nearest => "f32.nearest",
        F32Sqrt => "f32.sqrt",
        F32Add => "f32.add",
        F32Sub => "f32.sub",
        F32Mul => "f32.mul",
        F32Div => "f32.div",
        F32Min => "f32.min",
        F32Max => "f32.max",
        F32Copysign => "f32.copysign",

        F64Abs => "f64.abs",
        F64Neg => "f64.neg",
        F64Ceil => "f64.ceil",
        F64Floor => "f64.floor",
        F64Trunc => "f64.trunc",
        F64Nearest => "f64.nearest",
        F64Sqrt => "f64.sqrt",
        F64Add => "f64.add",
        F64Sub => "f64.sub",
        F64Mul => "f64.mul",
        F64Div => "f64.div",
        F64Min => "f64.min",
        F64Max => "f64.max",
        F64Copysign => "f64.copysign",

        I32WrapI64 => "i32.wrap_i64",
        I32TruncF32S => "i32.trunc_f32_s",
        I32TruncF32U => "i32.trunc_f32_u",
        I32TruncF64S => "i32.trunc_f64_s",
        I32TruncF64U => "i32.trunc_f64_u",
        I64ExtendI32S => "i64.extend_i32_s",
        I64ExtendI32U => "i64.extend_i32_u",
        I64TruncF32S => "i64.trunc_f32_s",
        I64TruncF32U => "i64.trunc_f32_u",
        I64TruncF64S => "i64.trunc_f64_s",
        I64TruncF64U => "i64.trunc_f64_u",

        F32ConvertI32S => "f32.convert_i32_s",
        F32ConvertI32U => "f32.convert_i32_u",
        F32ConvertI64S => "f32.convert_i64_s",
        F32ConvertI64U => "f32.convert_i64_u",
        F32DemoteF64 => "f32.demote_f64",
        F64ConvertI32S => "f64.convert_i32_s",
        F64ConvertI32U => "f64.convert_i32_u",
        F64ConvertI64S => "f64.convert_i64_s",
        F64ConvertI64U => "f64.convert_i64_u",
        F64PromoteF32 => "f64.promote_f32",

        I32ReinterpretF32 => "i32.reinterpret_f32",
        I64ReinterpretF64 => "i64.reinterpret_f64",
        F32ReinterpretI32 => "f32.reinterpret_i32",
        F64ReinterpretI64 => "f64.reinterpret_i64",

        I32Extend8S => "i32.extend8_s",
        I32Extend16S => "i32.extend16_s",
        I64Extend8S => "i64.extend8_s",
        I64Extend16S => "i64.extend16_s",
        I64Extend32S => "i64.extend32_s",

        I32TruncSatF32S => "i32.trunc_sat_f32_s",
        I32TruncSatF32U => "i32.trunc_sat_f32_u",
        I32TruncSatF64S => "i32.trunc_sat_f64_s",
        I32TruncSatF64U => "i32.trunc_sat_f64_u",
        I64TruncSatF32S => "i64.trunc_sat_f32_s",
        I64TruncSatF32U => "i64.trunc_sat_f32_u",
        I64TruncSatF64S => "i64.trunc_sat_f64_s",
        I64TruncSatF64U => "i64.trunc_sat_f64_u",

        MemoryInit { segment, mem: 0 } => "memory.init",
        MemoryInit { segment, mem } => "memory.init",
        DataDrop { segment } => "data.drop",
        MemoryCopy { src: 0, dst: 0 } => "memory.copy",
        MemoryCopy { src, dst } => "memory.copy",
        MemoryFill { mem: 0 } => "memory.fill",
        MemoryFill { mem } => "memory.fill",

        TableInit { table, segment } => "table.init",
        ElemDrop { segment } => "elem.drop",
        TableCopy { dst_table, src_table } => "table.copy",
        TableGet { table } => "table.get",
        TableSet { table } => "table.set",
        TableGrow { table } => "table.grow",
        TableSize { table } => "table.size",
        TableFill { table } => "table.fill",

        MemoryAtomicNotify { memarg } => "memory.atomic.notify",
        MemoryAtomicWait32 { memarg } => "memory.atomic.wait32",
        MemoryAtomicWait64 { memarg } => "memory.atomic.wait64",
        AtomicFence { flags: _ } => "atomic.fence",

        I32AtomicLoad { memarg } => "i32.atomic.load",
        I64AtomicLoad { memarg } => "i64.atomic.load",
        I32AtomicLoad8U { memarg } => "i32.atomic.load8_u",
        I32AtomicLoad16U { memarg } => "i32.atomic.load16_u",
        I64AtomicLoad8U { memarg } => "i64.atomic.load8_u",
        I64AtomicLoad16U { memarg } => "i64.atomic.load16_u",
        I64AtomicLoad32U { memarg } => "i64.atomic.load32_u",

        I32AtomicStore { memarg } => "i32.atomic.store",
        I64AtomicStore { memarg } => "i64.atomic.store",
        I32AtomicStore8 { memarg } => "i32.atomic.store8",
        I32AtomicStore16 { memarg } => "i32.atomic.store16",
        I64AtomicStore8 { memarg } => "i64.atomic.store8",
        I64AtomicStore16 { memarg } => "i64.atomic.store16",
        I64AtomicStore32 { memarg } => "i64.atomic.store32",

        I32AtomicRmwAdd { memarg } => "i32.atomic.rmw.add",
        I64AtomicRmwAdd { memarg } => "i64.atomic.rmw.add",
        I32AtomicRmw8AddU { memarg } => "i32.atomic.rmw8.add_u",
        I32AtomicRmw16AddU { memarg } => "i32.atomic.rmw16.add_u",
        I64AtomicRmw8AddU { memarg } => "i64.atomic.rmw8.add_u",
        I64AtomicRmw16AddU { memarg } => "i64.atomic.rmw16.add_u",
        I64AtomicRmw32AddU { memarg } => "i64.atomic.rmw32.add_u",

        I32AtomicRmwSub { memarg } => "i32.atomic.rmw.sub",
        I64AtomicRmwSub { memarg } => "i64.atomic.rmw.sub",
        I32AtomicRmw8SubU { memarg } => "i32.atomic.rmw8.sub_u",
        I32AtomicRmw16SubU { memarg } => "i32.atomic.rmw16.sub_u",
        I64AtomicRmw8SubU { memarg } => "i64.atomic.rmw8.sub_u",
        I64AtomicRmw16SubU { memarg } => "i64.atomic.rmw16.sub_u",
        I64AtomicRmw32SubU { memarg } => "i64.atomic.rmw32.sub_u",

        I32AtomicRmwAnd { memarg } => "i32.atomic.rmw.and",
        I64AtomicRmwAnd { memarg } => "i64.atomic.rmw.and",
        I32AtomicRmw8AndU { memarg } => "i32.atomic.rmw8.and_u",
        I32AtomicRmw16AndU { memarg } => "i32.atomic.rmw16.and_u",
        I64AtomicRmw8AndU { memarg } => "i64.atomic.rmw8.and_u",
        I64AtomicRmw16AndU { memarg } => "i64.atomic.rmw16.and_u",
        I64AtomicRmw32AndU { memarg } => "i64.atomic.rmw32.and_u",

        I32AtomicRmwOr { memarg } => "i32.atomic.rmw.or",
        I64AtomicRmwOr { memarg } => "i64.atomic.rmw.or",
        I32AtomicRmw8OrU { memarg } => "i32.atomic.rmw8.or_u",
        I32AtomicRmw16OrU { memarg } => "i32.atomic.rmw16.or_u",
        I64AtomicRmw8OrU { memarg } => "i64.atomic.rmw8.or_u",
        I64AtomicRmw16OrU { memarg } => "i64.atomic.rmw16.or_u",
        I64AtomicRmw32OrU { memarg } => "i64.atomic.rmw32.or_u",

        I32AtomicRmwXor { memarg } => "i32.atomic.rmw.xor",
        I64AtomicRmwXor { memarg } => "i64.atomic.rmw.xor",
        I32AtomicRmw8XorU { memarg } => "i32.atomic.rmw8.xor_u",
        I32AtomicRmw16XorU { memarg } => "i32.atomic.rmw16.xor_u",
        I64AtomicRmw8XorU { memarg } => "i64.atomic.rmw8.xor_u",
        I64AtomicRmw16XorU { memarg } => "i64.atomic.rmw16.xor_u",
        I64AtomicRmw32XorU { memarg } => "i64.atomic.rmw32.xor_u",

        I32AtomicRmwXchg { memarg } => "i32.atomic.rmw.xchg",
        I64AtomicRmwXchg { memarg } => "i64.atomic.rmw.xchg",
        I32AtomicRmw8XchgU { memarg } => "i32.atomic.rmw8.xchg_u",
        I32AtomicRmw16XchgU { memarg } => "i32.atomic.rmw16.xchg_u",
        I64AtomicRmw8XchgU { memarg } => "i64.atomic.rmw8.xchg_u",
        I64AtomicRmw16XchgU { memarg } => "i64.atomic.rmw16.xchg_u",
        I64AtomicRmw32XchgU { memarg } => "i64.atomic.rmw32.xchg_u",

        I32AtomicRmwCmpxchg { memarg } => "i32.atomic.rmw.cmpxchg",
        I64AtomicRmwCmpxchg { memarg } => "i64.atomic.rmw.cmpxchg",
        I32AtomicRmw8CmpxchgU { memarg } => "i32.atomic.rmw8.cmpxchg_u",
        I32AtomicRmw16CmpxchgU { memarg } => "i32.atomic.rmw16.cmpxchg_u",
        I64AtomicRmw8CmpxchgU { memarg } => "i64.atomic.rmw8.cmpxchg_u",
        I64AtomicRmw16CmpxchgU { memarg } => "i64.atomic.rmw16.cmpxchg_u",
        I64AtomicRmw32CmpxchgU { memarg } => "i64.atomic.rmw32.cmpxchg_u",

        V128Load { memarg } => "v128.load",
        V128Store { memarg } => "v128.store",
        V128Const { value } => "v128.const i32x4",

        I8x16Splat => "i8x16.splat",
        I8x16ExtractLaneS { lane } => "i8x16.extract_lane_s",
        I8x16ExtractLaneU { lane } => "i8x16.extract_lane_u",
        I8x16ReplaceLane { lane } => "i8x16.replace_lane",
        I16x8Splat => "i16x8.splat",
        I16x8ExtractLaneS { lane } => "i16x8.extract_lane_s",
        I16x8ExtractLaneU { lane } => "i16x8.extract_lane_u",
        I16x8ReplaceLane { lane } => "i16x8.replace_lane",
        I32x4Splat => "i32x4.splat",
        I32x4ExtractLane { lane } => "i32x4.extract_lane",
        I32x4ReplaceLane { lane } => "i32x4.replace_lane",
        I64x2Splat => "i64x2.splat",
        I64x2ExtractLane { lane } => "i64x2.extract_lane",
        I64x2ReplaceLane { lane } => "i64x2.replace_lane",
        F32x4Splat => "f32x4.splat",
        F32x4ExtractLane { lane } => "f32x4.extract_lane",
        F32x4ReplaceLane { lane } => "f32x4.replace_lane",
        F64x2Splat => "f64x2.splat",
        F64x2ExtractLane { lane } => "f64x2.extract_lane",
        F64x2ReplaceLane { lane } => "f64x2.replace_lane",

        I8x16Eq => "i8x16.eq",
        I8x16Ne => "i8x16.ne",
        I8x16LtS => "i8x16.lt_s",
        I8x16LtU => "i8x16.lt_u",
        I8x16GtS => "i8x16.gt_s",
        I8x16GtU => "i8x16.gt_u",
        I8x16LeS => "i8x16.le_s",
        I8x16LeU => "i8x16.le_u",
        I8x16GeS => "i8x16.ge_s",
        I8x16GeU => "i8x16.ge_u",

        I16x8Eq => "i16x8.eq",
        I16x8Ne => "i16x8.ne",
        I16x8LtS => "i16x8.lt_s",
        I16x8LtU => "i16x8.lt_u",
        I16x8GtS => "i16x8.gt_s",
        I16x8GtU => "i16x8.gt_u",
        I16x8LeS => "i16x8.le_s",
        I16x8LeU => "i16x8.le_u",
        I16x8GeS => "i16x8.ge_s",
        I16x8GeU => "i16x8.ge_u",

        I32x4Eq => "i32x4.eq",
        I32x4Ne => "i32x4.ne",
        I32x4LtS => "i32x4.lt_s",
        I32x4LtU => "i32x4.lt_u",
        I32x4GtS => "i32x4.gt_s",
        I32x4GtU => "i32x4.gt_u",
        I32x4LeS => "i32x4.le_s",
        I32x4LeU => "i32x4.le_u",
        I32x4GeS => "i32x4.ge_s",
        I32x4GeU => "i32x4.ge_u",

        I64x2Eq => "i64x2.eq",
        I64x2Ne => "i64x2.ne",
        I64x2LtS => "i64x2.lt_s",
        I64x2GtS => "i64x2.gt_s",
        I64x2LeS => "i64x2.le_s",
        I64x2GeS => "i64x2.ge_s",

        F32x4Eq => "f32x4.eq",
        F32x4Ne => "f32x4.ne",
        F32x4Lt => "f32x4.lt",
        F32x4Gt => "f32x4.gt",
        F32x4Le => "f32x4.le",
        F32x4Ge => "f32x4.ge",

        F64x2Eq => "f64x2.eq",
        F64x2Ne => "f64x2.ne",
        F64x2Lt => "f64x2.lt",
        F64x2Gt => "f64x2.gt",
        F64x2Le => "f64x2.le",
        F64x2Ge => "f64x2.ge",

        V128Not => "v128.not",
        V128And => "v128.and",
        V128AndNot => "v128.andnot",
        V128Or => "v128.or",
        V128Xor => "v128.xor",
        V128Bitselect => "v128.bitselect",
        V128AnyTrue => "v128.any_true",

        I8x16Abs => "i8x16.abs",
        I8x16Neg => "i8x16.neg",
        I8x16AllTrue => "i8x16.all_true",
        I8x16Bitmask => "i8x16.bitmask",
        I8x16Shl => "i8x16.shl",
        I8x16ShrU => "i8x16.shr_u",
        I8x16ShrS => "i8x16.shr_s",
        I8x16Add => "i8x16.add",
        I8x16AddSatS => "i8x16.add_sat_s",
        I8x16AddSatU => "i8x16.add_sat_u",
        I8x16Sub => "i8x16.sub",
        I8x16SubSatS => "i8x16.sub_sat_s",
        I8x16SubSatU => "i8x16.sub_sat_u",

        I16x8Abs => "i16x8.abs",
        I16x8Neg => "i16x8.neg",
        I16x8AllTrue => "i16x8.all_true",
        I16x8Bitmask => "i16x8.bitmask",
        I16x8Shl => "i16x8.shl",
        I16x8ShrU => "i16x8.shr_u",
        I16x8ShrS => "i16x8.shr_s",
        I16x8Add => "i16x8.add",
        I16x8AddSatS => "i16x8.add_sat_s",
        I16x8AddSatU => "i16x8.add_sat_u",
        I16x8Sub => "i16x8.sub",
        I16x8SubSatS => "i16x8.sub_sat_s",
        I16x8SubSatU => "i16x8.sub_sat_u",
        I16x8Mul => "i16x8.mul",

        I32x4Abs => "i32x4.abs",
        I32x4Neg => "i32x4.neg",
        I32x4AllTrue => "i32x4.all_true",
        I32x4Bitmask => "i32x4.bitmask",
        I32x4Shl => "i32x4.shl",
        I32x4ShrU => "i32x4.shr_u",
        I32x4ShrS => "i32x4.shr_s",
        I32x4Add => "i32x4.add",
        I32x4Sub => "i32x4.sub",
        I32x4Mul => "i32x4.mul",

        I64x2Abs => "i64x2.abs",
        I64x2Neg => "i64x2.neg",
        I64x2AllTrue => "i64x2.all_true",
        I64x2Bitmask => "i64x2.bitmask",
        I64x2Shl => "i64x2.shl",
        I64x2ShrU => "i64x2.shr_u",
        I64x2ShrS => "i64x2.shr_s",
        I64x2Add => "i64x2.add",
        I64x2Sub => "i64x2.sub",
        I64x2Mul => "i64x2.mul",

        F32x4Ceil => "f32x4.ceil",
        F32x4Floor => "f32x4.floor",
        F32x4Trunc => "f32x4.trunc",
        F32x4Nearest => "f32x4.nearest",
        F64x2Ceil => "f64x2.ceil",
        F64x2Floor => "f64x2.floor",
        F64x2Trunc => "f64x2.trunc",
        F64x2Nearest => "f64x2.nearest",
        F32x4Abs => "f32x4.abs",
        F32x4Neg => "f32x4.neg",
        F32x4Sqrt => "f32x4.sqrt",
        F32x4Add => "f32x4.add",
        F32x4Sub => "f32x4.sub",
        F32x4Div => "f32x4.div",
        F32x4Mul => "f32x4.mul",
        F32x4Min => "f32x4.min",
        F32x4Max => "f32x4.max",
        F32x4PMin => "f32x4.pmin",
        F32x4PMax => "f32x4.pmax",

        F64x2Abs => "f64x2.abs",
        F64x2Neg => "f64x2.neg",
        F64x2Sqrt => "f64x2.sqrt",
        F64x2Add => "f64x2.add",
        F64x2Sub => "f64x2.sub",
        F64x2Div => "f64x2.div",
        F64x2Mul => "f64x2.mul",
        F64x2Min => "f64x2.min",
        F64x2Max => "f64x2.max",
        F64x2PMin => "f64x2.pmin",
        F64x2PMax => "f64x2.pmax",

        I32x4TruncSatF32x4S => "i32x4.trunc_sat_f32x4_s",
        I32x4TruncSatF32x4U => "i32x4.trunc_sat_f32x4_u",
        F32x4ConvertI32x4S => "f32x4.convert_i32x4_s",
        F32x4ConvertI32x4U => "f32x4.convert_i32x4_u",

        I8x16Swizzle => "i8x16.swizzle",
        I8x16Shuffle { lanes } => "i8x16.shuffle",
        V128Load8Splat { memarg } => "v128.load8_splat",
        V128Load16Splat { memarg } => "v128.load16_splat",
        V128Load32Splat { memarg } => "v128.load32_splat",
        V128Load64Splat { memarg } => "v128.load64_splat",

        V128Load32Zero { memarg } => "v128.load32_zero",
        V128Load64Zero { memarg } => "v128.load64_zero",

        I8x16NarrowI16x8S => "i8x16.narrow_i16x8_s",
        I8x16NarrowI16x8U => "i8x16.narrow_i16x8_u",
        I16x8NarrowI32x4S => "i16x8.narrow_i32x4_s",
        I16x8NarrowI32x4U => "i16x8.narrow_i32x4_u",

        I16x8WidenLowI8x16S => "i16x8.widen_low_i8x16_s",
        I16x8WidenHighI8x16S => "i16x8.widen_high_i8x16_s",
        I16x8WidenLowI8x16U => "i16x8.widen_low_i8x16_u",
        I16x8WidenHighI8x16U => "i16x8.widen_high_i8x16_u",
        I32x4WidenLowI16x8S => "i32x4.widen_low_i16x8_s",
        I32x4WidenHighI16x8S => "i32x4.widen_high_i16x8_s",
        I32x4WidenLowI16x8U => "i32x4.widen_low_i16x8_u",
        I32x4WidenHighI16x8U => "i32x4.widen_high_i16x8_u",
        I64x2WidenLowI32x4S => "i64x2.widen_low_i32x4_s",
        I64x2WidenHighI32x4S => "i64x2.widen_high_i32x4_s",
        I64x2WidenLowI32x4U => "i64x2.widen_low_i32x4_u",
        I64x2WidenHighI32x4U => "i64x2.widen_high_i32x4_u",

        I16x8ExtMulLowI8x16S => "i16x8.extmul_low_i8x16_s",
        I16x8ExtMulHighI8x16S => "i16x8.extmul_high_i8x16_s",
        I16x8ExtMulLowI8x16U => "i16x8.extmul_low_i8x16_u",
        I16x8ExtMulHighI8x16U => "i16x8.extmul_high_i8x16_u",
        I32x4ExtMulLowI16x8S => "i32x4.extmul_low_i16x8_s",
        I32x4ExtMulHighI16x8S => "i32x4.extmul_high_i16x8_s",
        I32x4ExtMulLowI16x8U => "i32x4.extmul_low_i16x8_u",
        I32x4ExtMulHighI16x8U => "i32x4.extmul_high_i16x8_u",
        I64x2ExtMulLowI32x4S => "i64x2.extmul_low_i32x4_s",
        I64x2ExtMulHighI32x4S => "i64x2.extmul_high_i32x4_s",
        I64x2ExtMulLowI32x4U => "i64x2.extmul_low_i32x4_u",
        I64x2ExtMulHighI32x4U => "i64x2.extmul_high_i32x4_u",

        I16x8Q15MulrSatS => "i16x8.q15mulr_sat_s",

        V128Load8x8S { memarg } => "v128.load8x8_s",
        V128Load8x8U { memarg } => "v128.load8x8_u",
        V128Load16x4S { memarg } => "v128.load16x4_s",
        V128Load16x4U { memarg } => "v128.load16x4_u",
        V128Load32x2S { memarg } => "v128.load32x2_s",
        V128Load32x2U { memarg } => "v128.load32x2_u",

        V128Load8Lane { memarg, lane } => "v128.load8_lane",
        V128Load16Lane { memarg, lane } => "v128.load16_lane",
        V128Load32Lane { memarg, lane } => "v128.load32_lane",
        V128Load64Lane { memarg, lane } => "v128.load64_lane",

        V128Store8Lane { memarg, lane } => "v128.store8_lane",
        V128Store16Lane { memarg, lane } => "v128.store16_lane",
        V128Store32Lane { memarg, lane } => "v128.store32_lane",
        V128Store64Lane { memarg, lane } => "v128.store64_lane",

        I8x16RoundingAverageU => "i8x16.avgr_u",
        I16x8RoundingAverageU => "i16x8.avgr_u",

        I8x16MinS => "i8x16.min_s",
        I8x16MinU => "i8x16.min_u",
        I8x16MaxS => "i8x16.max_s",
        I8x16MaxU => "i8x16.max_u",
        I16x8MinS => "i16x8.min_s",
        I16x8MinU => "i16x8.min_u",
        I16x8MaxS => "i16x8.max_s",
        I16x8MaxU => "i16x8.max_u",
        I32x4MinS => "i32x4.min_s",
        I32x4MinU => "i32x4.min_u",
        I32x4MaxS => "i32x4.max_s",
        I32x4MaxU => "i32x4.max_u",
        I32x4DotI16x8S => "i32x4.dot_i16x8_s",

        F32x4DemoteF64x2Zero => "f32x4.demote_f64x2_zero",
        F64x2PromoteLowF32x4 => "f64x2.promote_low_f32x4",
        F64x2ConvertLowI32x4S => "f64x2.convert_low_i32x4_s",
        F64x2ConvertLowI32x4U => "f64x2.convert_low_i32x4_u",
        I32x4TruncSatF64x2SZero => "i32x4.trunc_sat_f64x2_s_zero",
        I32x4TruncSatF64x2UZero => "i32x4.trunc_sat_f64x2_u_zero",

        I8x16Popcnt => "i8x16.popcnt",

        I16x8ExtAddPairwiseI8x16S => "i16x8.extadd_pairwise_i8x16_s",
        I16x8ExtAddPairwiseI8x16U => "i16x8.extadd_pairwise_i8x16_u",
        I32x4ExtAddPairwiseI16x8S => "i32x4.extadd_pairwise_i16x8_s",
        I32x4ExtAddPairwiseI16x8U => "i32x4.extadd_pairwise_i16x8_u",
    }
}
