use crate::types::{AsTypeRef, BasicTypeEnum, FloatType, IntType};
use libc::c_void;
use llvm_sys::execution_engine::{
    LLVMCreateAggregateGenericValue, LLVMCreateGenericValueOfData, LLVMCreateGenericValueOfFloat,
    LLVMCreateGenericValueOfFloatDouble, LLVMCreateGenericValueOfFloatSingle, LLVMCreateGenericValueOfInt,
    LLVMCreateGenericValueOfMiriPointer, LLVMCreateGenericValueOfPointer, LLVMDisposeGenericValue,
    LLVMGenericValueAppendAggregateValue, LLVMGenericValueArrayRef, LLVMGenericValueArrayRefGetElementAt,
    LLVMGenericValueArrayRefLength, LLVMGenericValueEnsureCapacity, LLVMGenericValueGetTypeTag,
    LLVMGenericValueIntWidth, LLVMGenericValueRef, LLVMGenericValueSetDataValue, LLVMGenericValueSetDoubleValue,
    LLVMGenericValueSetFloatValue, LLVMGenericValueSetIntValue, LLVMGenericValueSetMiriPointerValue,
    LLVMGenericValueSetTypeTag, LLVMGenericValueToFloat, LLVMGenericValueToFloatDouble, LLVMGenericValueToFloatSingle,
    LLVMGenericValueToInt, LLVMGenericValueToMiriPointer, LLVMGenericValueToPointer,
    LLVMGetAggregateGenericValueLength, LLVMGetPointerToAggregateGenericValue,
};
pub use llvm_sys::miri::MiriPointer;
use std::{marker::PhantomData, mem};

//A version of GenericValue that isn't reponsible for dropping the LLVMGenericValueRef
//This is used in the ExecutionEngine to avoid double frees
#[derive(Debug, Copy, Clone)]
pub struct GenericValueRef<'ctx> {
    pub(crate) generic_value: LLVMGenericValueRef,
    _phantom: PhantomData<&'ctx ()>,
}

impl<'ctx> GenericValueRef<'ctx> {
    pub unsafe fn new(generic_value: LLVMGenericValueRef) -> GenericValueRef<'static> {
        assert!(!generic_value.is_null());
        GenericValueRef {
            generic_value,
            _phantom: PhantomData,
        }
    }

    #[inline]
    fn get_field_type(&self, index: u64) -> Option<BasicTypeEnum<'ctx>> {
        match self.get_type_tag() {
            Some(BasicTypeEnum::ArrayType(at)) => Some(at.get_element_type()),
            Some(BasicTypeEnum::StructType(st)) => st.get_field_types().get(usize::try_from(index).unwrap()).copied(),
            Some(BasicTypeEnum::VectorType(vt)) => Some(vt.get_element_type()),
            _ => None,
        }
    }

    pub fn assert_fields(&self) -> Vec<GenericValueRef<'ctx>> {
        self.get_fields().unwrap()
    }

    pub fn get_fields(&self) -> Option<Vec<GenericValueRef<'ctx>>> {
        let field_array = (0..self.get_aggregate_size())
            .map(|idx| self.get_field_type(idx))
            .collect::<Option<Vec<BasicTypeEnum<'ctx>>>>();
        if let Some(field_array) = field_array {
            return field_array
                .iter()
                .enumerate()
                .map(|(idx, at)| {
                    let idx_cvt = idx.try_into().unwrap();
                    let field = unsafe {
                        GenericValueRef::new(LLVMGetPointerToAggregateGenericValue(self.generic_value, idx_cvt))
                    };
                    field.set_type_tag(at);
                    Some(field)
                })
                .collect::<Option<Vec<GenericValueRef<'ctx>>>>();
        }
        None
    }

    pub fn get_type_tag(&self) -> Option<BasicTypeEnum<'ctx>> {
        let type_tag = unsafe { LLVMGenericValueGetTypeTag(self.generic_value) };
        if type_tag == std::ptr::null_mut() {
            None
        } else {
            Some(unsafe { BasicTypeEnum::new(type_tag) })
        }
    }

    pub fn assert_type_tag(&self) -> BasicTypeEnum<'ctx> {
        self.get_type_tag().unwrap()
    }

    pub fn set_type_tag(&self, bte: &BasicTypeEnum<'ctx>) {
        unsafe {
            LLVMGenericValueSetTypeTag(self.generic_value, bte.as_type_ref());
        }
    }

    pub fn as_miri_pointer(&self) -> MiriPointer {
        unsafe { LLVMGenericValueToMiriPointer(self.generic_value) }
    }

    // SubType: GenericValue<IntValue> only
    pub fn int_width(&self) -> u32 {
        unsafe { LLVMGenericValueIntWidth(self.generic_value) }
    }
    pub fn int_width_bytes(&self) -> u32 {
        unsafe { (LLVMGenericValueIntWidth(self.generic_value) + 7) / 8 }
    }

    // SubType: impl only for GenericValue<IntValue>
    pub fn as_int(&self) -> u128 {
        let apint_pointer = unsafe { LLVMGenericValueToInt(self.generic_value) };
        let apint_slice = unsafe { std::slice::from_raw_parts(apint_pointer.data, apint_pointer.words as usize) };
        assert!(
            apint_slice.len() <= 2,
            "GenericValue::as_int() supports values of a maximum size of 128 bits."
        );
        let apint_byte_slice = apint_slice
            .iter()
            .flat_map(|x| x.to_ne_bytes().to_vec())
            .collect::<Vec<u8>>();
        if apint_slice.len() > 1 {
            u128::from_ne_bytes(apint_byte_slice.try_into().unwrap())
        } else {
            u64::from_ne_bytes(apint_byte_slice.try_into().unwrap()).into()
        }
    }

    // SubType: impl only for GenericValue<FloatValue>
    pub fn as_float(&self, float_type: &FloatType<'_>) -> f64 {
        unsafe { LLVMGenericValueToFloat(float_type.as_type_ref(), self.generic_value) }
    }

    pub fn as_f32(&self) -> f32 {
        unsafe { LLVMGenericValueToFloatSingle(self.generic_value) }
    }

    pub fn as_f64(&self) -> f64 {
        unsafe { LLVMGenericValueToFloatDouble(self.generic_value) }
    }

    pub fn set_float_value(&mut self, float: f32) {
        unsafe { LLVMGenericValueSetFloatValue(self.generic_value, float) }
    }

    pub fn set_double_value(&mut self, double: f64) {
        unsafe { LLVMGenericValueSetDoubleValue(self.generic_value, double) }
    }

    pub fn set_int_value(&mut self, value: u128, bytes: u64) {
        let byte_array = value.to_ne_bytes();
        let value_slice = byte_array
            .chunks_exact(8)
            .map(|x| u64::from_ne_bytes(x.try_into().unwrap()))
            .collect::<Vec<u64>>();
        unsafe { LLVMGenericValueSetIntValue(self.generic_value, value_slice.as_ptr(), bytes) }
    }

    pub fn set_bytes(&mut self, bytes: &[u8]) {
        unsafe { LLVMGenericValueSetDataValue(self.generic_value, bytes.as_ptr(), bytes.len().try_into().unwrap()) }
    }

    pub fn set_miri_pointer_value(&mut self, value: MiriPointer) {
        unsafe { LLVMGenericValueSetMiriPointerValue(self.generic_value, value) }
    }

    pub fn append_aggregate_value(&mut self, val: GenericValue<'_>) {
        let val = unsafe { val.into_raw() };
        unsafe {
            LLVMGenericValueAppendAggregateValue(self.generic_value, val);
        }
    }

    pub fn get_aggregate_size(&self) -> u64 {
        unsafe { LLVMGetAggregateGenericValueLength(self.generic_value) as u64 }
    }

    pub fn assert_field(&self, index: u64) -> GenericValueRef<'ctx> {
        if let Some(field) = self.get_field(index) {
            field
        } else {
            panic!("GenericValue::assert_field() index out of bounds");
        }
    }

    pub fn get_field(&self, index: u64) -> Option<GenericValueRef<'ctx>> {
        let len = self.get_aggregate_size();
        if let Some(field_type) = self.get_field_type(index) {
            if index < len {
                let field =
                    unsafe { GenericValueRef::new(LLVMGetPointerToAggregateGenericValue(self.generic_value, index)) };
                field.set_type_tag(&field_type);
                return Some(field);
            }
        }
        None
    }
    pub fn ensure_capacity(&self, capacity: u64) {
        unsafe {
            LLVMGenericValueEnsureCapacity(self.generic_value, capacity);
        }
    }

    // SubType: impl only for GenericValue<PointerValue, T>
    // REVIEW: How safe is this really?
    pub unsafe fn into_pointer<T>(&self) -> *mut T {
        LLVMGenericValueToPointer(self.generic_value) as *mut T
    }

    pub fn into_raw(self) -> LLVMGenericValueRef {
        self.generic_value
    }
}

#[derive(Debug)]
pub struct GenericValue<'ctx> {
    pub(crate) generic_value_ref: GenericValueRef<'ctx>,
    _phantom: PhantomData<&'ctx ()>,
}

impl<'ctx> GenericValue<'ctx> {
    pub(crate) unsafe fn new(generic_value: LLVMGenericValueRef) -> Self {
        assert!(!generic_value.is_null());
        GenericValue {
            generic_value_ref: GenericValueRef::new(generic_value),
            _phantom: PhantomData,
        }
    }

    pub fn new_void() -> Self {
        unsafe {
            let empty = [0];
            let v_ref = LLVMCreateGenericValueOfData(empty.as_ptr(), empty.len().try_into().unwrap());
            GenericValue::new(v_ref)
        }
    }

    pub fn new_aggregate(members: u64) -> Self {
        unsafe {
            let value = LLVMCreateAggregateGenericValue(members);
            GenericValue::new(value)
        }
    }

    pub fn new_float(value: f64, float_type: &FloatType<'ctx>) -> Self {
        unsafe {
            let v_ref = LLVMCreateGenericValueOfFloat(float_type.as_type_ref(), value);
            GenericValue::new(v_ref)
        }
    }

    pub fn new_f32(value: f32) -> Self {
        unsafe {
            let v_ref = LLVMCreateGenericValueOfFloatSingle(value);
            GenericValue::new(v_ref)
        }
    }

    pub fn new_f64(value: f64) -> Self {
        unsafe {
            let v_ref = LLVMCreateGenericValueOfFloatDouble(value);
            GenericValue::new(v_ref)
        }
    }

    pub fn new_int(value: u64, int_type: &IntType<'ctx>, is_signed: bool) -> Self {
        unsafe {
            let v_ref = LLVMCreateGenericValueOfInt(int_type.as_type_ref(), value, is_signed as i32);
            GenericValue::new(v_ref)
        }
    }

    pub fn from_byte_slice(bytes: &[u8]) -> Self {
        unsafe {
            let value = LLVMCreateGenericValueOfData(bytes.as_ptr() as *const u8, bytes.len() as u32);
            GenericValue::new(value)
        }
    }

    // SubType: create_generic_value() -> GenericValue<PointerValue, T>
    // REVIEW: How safe is this really?
    pub unsafe fn create_generic_value_of_pointer<T>(value: &mut T) -> Self {
        let value = LLVMCreateGenericValueOfPointer(value as *mut _ as *mut c_void);
        GenericValue::new(value)
    }

    pub unsafe fn create_generic_value_of_miri_pointer(meta: MiriPointer) -> Self {
        let value = LLVMCreateGenericValueOfMiriPointer(meta);
        GenericValue::new(value)
    }

    pub fn as_ref(&self) -> &GenericValueRef<'ctx> {
        &self.generic_value_ref
    }

    pub fn as_mut(&mut self) -> &mut GenericValueRef<'ctx> {
        &mut self.generic_value_ref
    }

    pub unsafe fn into_raw(self) -> LLVMGenericValueRef {
        let value = self.generic_value_ref.generic_value;
        mem::forget(self);
        value
    }
    pub unsafe fn from_raw(value: LLVMGenericValueRef) -> Self {
        unsafe { GenericValue::new(value) }
    }
}

impl Drop for GenericValue<'_> {
    fn drop(&mut self) {
        unsafe { LLVMDisposeGenericValue(self.generic_value_ref.generic_value) }
    }
}

#[derive(Debug)]
pub struct GenericValueArrayRef<'ctx> {
    pub(crate) generic_value_array: LLVMGenericValueArrayRef,
    _phantom: PhantomData<&'ctx ()>,
}

impl<'ctx> GenericValueArrayRef<'ctx> {
    pub unsafe fn new(array_ref: LLVMGenericValueArrayRef) -> GenericValueArrayRef<'static> {
        assert!(!array_ref.is_null());

        GenericValueArrayRef {
            generic_value_array: array_ref,
            _phantom: PhantomData,
        }
    }

    pub fn len(&self) -> u64 {
        unsafe { LLVMGenericValueArrayRefLength(self.generic_value_array) }
    }

    pub fn get_element_at(&self, index: u64) -> Option<GenericValueRef<'ctx>> {
        let len = self.len();
        unsafe {
            if index < len as u64 {
                Some(GenericValueRef::new(LLVMGenericValueArrayRefGetElementAt(
                    self.generic_value_array,
                    index,
                )))
            } else {
                None
            }
        }
    }
}
