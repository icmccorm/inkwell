use llvm_sys::core::{LLVMAlignOf, LLVMArrayType, LLVMConstArray, LLVMConstInt, LLVMConstNamedStruct, LLVMConstReal, LLVMCountParamTypes, LLVMDumpType, LLVMFunctionType, LLVMGetParamTypes, LLVMGetTypeContext, LLVMGetTypeKind, LLVMGetUndef, LLVMIsFunctionVarArg, LLVMPointerType, LLVMPrintTypeToString, LLVMStructGetTypeAtIndex, LLVMTypeIsSized, LLVMInt1Type, LLVMInt8Type, LLVMInt16Type, LLVMInt32Type, LLVMInt64Type, LLVMIntType, LLVMGetArrayLength, LLVMSizeOf, LLVMIsPackedStruct, LLVMIsOpaqueStruct, LLVMHalfType, LLVMFloatType, LLVMDoubleType, LLVMFP128Type, LLVMGetIntTypeWidth, LLVMVoidType, LLVMStructType, LLVMCountStructElementTypes, LLVMGetStructElementTypes, LLVMGetPointerAddressSpace, LLVMVectorType, LLVMGetVectorSize, LLVMConstVector, LLVMPPCFP128Type, LLVMGetStructName, LLVMConstAllOnes, LLVMConstPointerNull, LLVMConstNull};
use llvm_sys::prelude::{LLVMTypeRef, LLVMValueRef};
use llvm_sys::LLVMTypeKind;

use std::ffi::CStr;
use std::fmt;
use std::mem::forget;

use context::{Context, ContextRef};
use values::{AsValueRef, ArrayValue, BasicValue, FloatValue, IntValue, PointerValue, StructValue, VectorValue, Value}; // TODO: Remove Value

mod private {
    // This is an ugly privacy hack so that Type can stay private to this module
    // and so that super traits using this trait will be not be implementable
    // outside this library
    use llvm_sys::prelude::LLVMTypeRef;

    pub trait AsTypeRef {
        fn as_type_ref(&self) -> LLVMTypeRef;
    }
}

pub(crate) use self::private::AsTypeRef;

// Worth noting that types seem to be singletons. At the very least, primitives are.
// Though this is likely only true per thread since LLVM claims to not be very thread-safe.
// TODO: Make not public if possible
struct Type {
    type_: LLVMTypeRef,
}

impl Type {
    fn new(type_: LLVMTypeRef) -> Self {
        assert!(!type_.is_null());

        Type {
            type_: type_,
        }
    }

    // NOTE: AnyType
    fn print_to_stderr(&self) {
        unsafe {
            LLVMDumpType(self.type_);
        }
    }

    fn const_null_ptr(&self) -> PointerValue {
        let ptr_type = unsafe {
            LLVMConstPointerNull(self.type_)
        };

        PointerValue::new(ptr_type)
    }

    fn ptr_type(&self, address_space: u32) -> PointerType {
        let ptr_type = unsafe {
            LLVMPointerType(self.type_, address_space)
        };

        PointerType::new(ptr_type)
    }

    fn vec_type(&self, size: u32) -> VectorType {
        let vec_type = unsafe {
            LLVMVectorType(self.type_, size)
        };

        VectorType::new(vec_type)
    }

    // REVIEW: Is this actually AnyType except FunctionType? VoidType? Can you make a FunctionType from a FunctionType???
    fn fn_type(&self, param_types: &[&AnyType], is_var_args: bool) -> FunctionType {
        let mut param_types: Vec<LLVMTypeRef> = param_types.iter()
                                                           .map(|val| val.as_type_ref())
                                                           .collect();
        let fn_type = unsafe {
            LLVMFunctionType(self.type_, param_types.as_mut_ptr(), param_types.len() as u32, is_var_args as i32)
        };

        FunctionType::new(fn_type)
    }

    fn array_type(&self, size: u32) -> ArrayType {
        let type_ = unsafe {
            LLVMArrayType(self.type_, size)
        };

        ArrayType::new(type_)
    }

    // NOTE: AnyType?
    // REVIEW: Untested; impl AnyValue?
    fn get_undef(&self) -> Value {
        let value = unsafe {
            LLVMGetUndef(self.type_)
        };

        Value::new(value)
    }

    // NOTE: AnyType
    pub(crate) fn get_kind(&self) -> LLVMTypeKind {
        unsafe {
            LLVMGetTypeKind(self.type_)
        }
    }

    // REVIEW: Untested; Return IntValue?
    fn get_alignment(&self) -> IntValue {
        let val = unsafe {
            LLVMAlignOf(self.type_)
        };

        IntValue::new(val)
    }

    fn get_context(&self) -> ContextRef {
        // We don't return an option because LLVM seems
        // to always assign a context, even to types
        // created without an explicit context, somehow

        let context = unsafe {
            LLVMGetTypeContext(self.type_)
        };

        ContextRef::new(Context::new(context))
    }

    fn is_sized(&self) -> bool {
        unsafe {
            LLVMTypeIsSized(self.type_) == 1
        }
    }

    fn size(&self) -> IntValue { // Option<IntValue>? What happens when type is unsized? We could return 0?
        let int_value = unsafe {
            LLVMSizeOf(self.type_)
        };

        IntValue::new(int_value)
    }

    fn print_to_string(&self) -> &CStr {
        unsafe {
            CStr::from_ptr(LLVMPrintTypeToString(self.type_))
        }
    }
}

impl fmt::Debug for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let llvm_type = self.print_to_string();

        write!(f, "Type {{\n    address: {:?}\n    llvm_type: {:?}\n}}", self.type_, llvm_type)
    }
}

pub struct FunctionType {
    fn_type: Type,
}

impl FunctionType {
    pub(crate) fn new(fn_type: LLVMTypeRef) -> FunctionType {
        assert!(!fn_type.is_null());

        FunctionType {
            fn_type: Type::new(fn_type)
        }
    }

    // REVIEW: Not working
    fn is_var_arg(&self) -> bool {
        unsafe {
            LLVMIsFunctionVarArg(self.fn_type.type_) != 0
        }
    }

    pub fn get_param_types(&self) -> Vec<BasicTypeEnum> {
        let count = self.count_param_types();
        let mut raw_vec: Vec<LLVMTypeRef> = Vec::with_capacity(count as usize);
        let ptr = raw_vec.as_mut_ptr();

        forget(raw_vec);

        let raw_vec = unsafe {
            LLVMGetParamTypes(self.fn_type.type_, ptr);

            Vec::from_raw_parts(ptr, count as usize, count as usize)
        };

        raw_vec.iter().map(|val| BasicTypeEnum::new(*val)).collect()
    }

    pub fn count_param_types(&self) -> u32 {
        unsafe {
            LLVMCountParamTypes(self.fn_type.type_)
        }
    }

    pub fn is_sized(&self) -> bool {
        self.fn_type.is_sized()
    }

    pub fn get_context(&self) -> ContextRef {
        self.fn_type.get_context()
    }

    pub fn print_to_string(&self) -> &CStr {
        self.fn_type.print_to_string()
    }

    pub fn print_to_stderr(&self) {
        self.fn_type.print_to_stderr()
    }
}

impl fmt::Debug for FunctionType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let llvm_type = self.print_to_string();

        write!(f, "FunctionType {{\n    address: {:?}\n    llvm_type: {:?}\n}}", self.fn_type.type_, llvm_type)
    }
}

impl AsTypeRef for FunctionType {
    fn as_type_ref(&self) -> LLVMTypeRef {
        self.fn_type.type_
    }
}

#[derive(Debug)]
pub struct IntType {
    int_type: Type,
}

impl IntType {
    pub(crate) fn new(int_type: LLVMTypeRef) -> Self {
        assert!(!int_type.is_null());

        IntType {
            int_type: Type::new(int_type),
        }
    }

    pub fn bool_type() -> Self {
        let type_ = unsafe {
            LLVMInt1Type()
        };

        IntType::new(type_)
    }

    pub fn i8_type() -> Self {
        let type_ = unsafe {
            LLVMInt8Type()
        };

        IntType::new(type_)
    }

    pub fn i16_type() -> Self {
        let type_ = unsafe {
            LLVMInt16Type()
        };

        IntType::new(type_)
    }

    pub fn i32_type() -> Self {
        let type_ = unsafe {
            LLVMInt32Type()
        };

        IntType::new(type_)
    }

    pub fn i64_type() -> Self {
        let type_ = unsafe {
            LLVMInt64Type()
        };

        IntType::new(type_)
    }

    pub fn i128_type() -> Self {
        // REVIEW: The docs says there's a LLVMInt128Type, but
        // it might only be in a newer version

        let type_ = unsafe {
            LLVMIntType(128)
        };

        IntType::new(type_)
    }

    pub fn custom_width_int_type(bits: u32) -> Self {
        let type_ = unsafe {
            LLVMIntType(bits)
        };

        IntType::new(type_)
    }

    pub fn const_int(&self, value: u64, sign_extend: bool) -> IntValue {
        let value = unsafe {
            LLVMConstInt(self.as_type_ref(), value, sign_extend as i32)
        };

        IntValue::new(value)
    }

    pub fn const_all_ones(&self) -> IntValue {
        let value = unsafe {
            LLVMConstAllOnes(self.as_type_ref())
        };

        IntValue::new(value)
    }

    pub fn const_null_ptr(&self) -> PointerValue {
        self.int_type.const_null_ptr()
    }

    pub fn const_null(&self) -> IntValue {
        let null = unsafe {
            LLVMConstNull(self.as_type_ref())
        };

        IntValue::new(null)
    }

    pub fn fn_type(&self, param_types: &[&AnyType], is_var_args: bool) -> FunctionType {
        self.int_type.fn_type(param_types, is_var_args)
    }

    pub fn array_type(&self, size: u32) -> ArrayType {
        self.int_type.array_type(size)
    }

    pub fn vec_type(&self, size: u32) -> VectorType {
        self.int_type.vec_type(size)
    }

    pub fn get_context(&self) -> ContextRef {
        self.int_type.get_context()
    }

    pub fn is_sized(&self) -> bool {
        self.int_type.is_sized()
    }

    pub fn ptr_type(&self, address_space: u32) -> PointerType {
        self.int_type.ptr_type(address_space)
    }

    pub fn get_bit_width(&self) -> u32 {
        unsafe {
            LLVMGetIntTypeWidth(self.as_type_ref())
        }
    }

    pub fn print_to_string(&self) -> &CStr {
        self.int_type.print_to_string()
    }

    pub fn print_to_stderr(&self) {
        self.int_type.print_to_stderr()
    }
}

impl AsTypeRef for IntType {
    fn as_type_ref(&self) -> LLVMTypeRef {
        self.int_type.type_
    }
}

#[derive(Debug)]
pub struct FloatType {
    float_type: Type,
}

impl FloatType {
    pub(crate) fn new(float_type: LLVMTypeRef) -> Self {
        assert!(!float_type.is_null());

        FloatType {
            float_type: Type::new(float_type),
        }
    }

    pub fn fn_type(&self, param_types: &[&AnyType], is_var_args: bool) -> FunctionType {
        self.float_type.fn_type(param_types, is_var_args)
    }

    pub fn array_type(&self, size: u32) -> ArrayType {
        self.float_type.array_type(size)
    }

    pub fn vec_type(&self, size: u32) -> VectorType {
        self.float_type.vec_type(size)
    }

    pub fn const_float(&self, value: f64) -> FloatValue {
        let value = unsafe {
            LLVMConstReal(self.float_type.type_, value)
        };

        FloatValue::new(value)
    }

    pub fn const_null_ptr(&self) -> PointerValue {
        self.float_type.const_null_ptr()
    }

    pub fn const_null(&self) -> FloatValue {
        let null = unsafe {
            LLVMConstNull(self.as_type_ref())
        };

        FloatValue::new(null)
    }

    pub fn is_sized(&self) -> bool {
        self.float_type.is_sized()
    }

    pub fn get_context(&self) -> ContextRef {
        self.float_type.get_context()
    }

    pub fn ptr_type(&self, address_space: u32) -> PointerType {
        self.float_type.ptr_type(address_space)
    }

    pub fn f16_type() -> Self {
        let float_type = unsafe {
            LLVMHalfType()
        };

        FloatType::new(float_type)
    }

    pub fn f32_type() -> Self {
        let float_type = unsafe {
            LLVMFloatType()
        };

        FloatType::new(float_type)
    }

    pub fn f64_type() -> Self {
        let float_type = unsafe {
            LLVMDoubleType()
        };

        FloatType::new(float_type)
    }

    pub fn i128_type() -> Self {
        let float_type = unsafe {
            LLVMFP128Type()
        };

        FloatType::new(float_type)
    }

    pub fn f128_type_ppc() -> Self {
        let float_type = unsafe {
            LLVMPPCFP128Type()
        };

        FloatType::new(float_type)
    }

    pub fn print_to_string(&self) -> &CStr {
        self.float_type.print_to_string()
    }

    pub fn print_to_stderr(&self) {
        self.float_type.print_to_stderr()
    }
}

impl AsTypeRef for FloatType {
    fn as_type_ref(&self) -> LLVMTypeRef {
        self.float_type.type_
    }
}

#[derive(Debug)]
pub struct StructType {
    struct_type: Type,
}

impl StructType {
    pub(crate) fn new(struct_type: LLVMTypeRef) -> Self {
        assert!(!struct_type.is_null());

        StructType {
            struct_type: Type::new(struct_type),
        }
    }

    // REVIEW: Untested
    // TODO: Would be great to be able to smartly be able to do this by field name
    // TODO: LLVM 3.7+ only
    pub fn get_type_at_field_index(&self, index: u32) -> Option<BasicTypeEnum> {
        // REVIEW: This should only be used on Struct Types, so add a StructType?
        let type_ = unsafe {
            LLVMStructGetTypeAtIndex(self.struct_type.type_, index)
        };

        if type_.is_null() {
            return None;
        }

        Some(BasicTypeEnum::new(type_))
    }

    // REVIEW: Untested
    // TODO: Better name for num. What's it for?
    pub fn const_struct(&self, value: &Value, num: u32) -> StructValue {
        let value = &mut [value.as_value_ref()];

        let val = unsafe {
            LLVMConstNamedStruct(self.struct_type.type_, value.as_mut_ptr(), num)
        };

        StructValue::new(val)
    }

    pub fn const_null_ptr(&self) -> PointerValue {
        self.struct_type.const_null_ptr()
    }

    pub fn const_null(&self) -> StructValue {
        let null = unsafe {
            LLVMConstNull(self.as_type_ref())
        };

        StructValue::new(null)
    }

    pub fn is_sized(&self) -> bool {
        self.struct_type.is_sized()
    }

    pub fn get_context(&self) -> ContextRef {
        self.struct_type.get_context()
    }

    pub fn get_name(&self) -> &CStr {
        unsafe {
            CStr::from_ptr(LLVMGetStructName(self.as_type_ref()))
        }
    }

    pub fn ptr_type(&self, address_space: u32) -> PointerType {
        self.struct_type.ptr_type(address_space)
    }

    pub fn fn_type(&self, param_types: &[&AnyType], is_var_args: bool) -> FunctionType {
        self.struct_type.fn_type(param_types, is_var_args)
    }

    pub fn array_type(&self, size: u32) -> ArrayType {
        self.struct_type.array_type(size)
    }

    pub fn is_packed(&self) -> bool {
        // REVIEW: Is == 1 correct?
        unsafe {
            LLVMIsPackedStruct(self.struct_type.type_) == 1
        }
    }

    pub fn is_opaque(&self) -> bool {
        // REVIEW: Is == 1 correct?
        unsafe {
            LLVMIsOpaqueStruct(self.struct_type.type_) == 1
        }
    }

    // REVIEW: No way to set name like in context.struct_type() method?
    pub fn struct_type(field_types: &[&BasicType], packed: bool) -> Self {
        let mut field_types: Vec<LLVMTypeRef> = field_types.iter()
                                                           .map(|val| val.as_type_ref())
                                                           .collect();
        let struct_type = unsafe {
            LLVMStructType(field_types.as_mut_ptr(), field_types.len() as u32, packed as i32)
        };

        StructType::new(struct_type)
    }

    // REVIEW: Method name
    pub fn count_field_types(&self) -> u32 {
        unsafe {
            LLVMCountStructElementTypes(self.as_type_ref())
        }
    }

    // REVIEW: Method name
    pub fn get_field_types(&self) -> Vec<BasicTypeEnum> {
        let count = self.count_field_types();
        let mut raw_vec: Vec<LLVMTypeRef> = Vec::with_capacity(count as usize);
        let ptr = raw_vec.as_mut_ptr();

        forget(raw_vec);

        let raw_vec = unsafe {
            LLVMGetStructElementTypes(self.as_type_ref(), ptr);

            Vec::from_raw_parts(ptr, count as usize, count as usize)
        };

        raw_vec.iter().map(|val| BasicTypeEnum::new(*val)).collect()
    }

    pub fn print_to_string(&self) -> &CStr {
        self.struct_type.print_to_string()
    }

    pub fn print_to_stderr(&self) {
        self.struct_type.print_to_stderr()
    }
}

impl AsTypeRef for StructType {
    fn as_type_ref(&self) -> LLVMTypeRef {
        self.struct_type.type_
    }
}

#[derive(Debug)]
pub struct VoidType {
    void_type: Type,
}

impl VoidType {
    pub(crate) fn new(void_type: LLVMTypeRef) -> Self {
        assert!(!void_type.is_null());

        VoidType {
            void_type: Type::new(void_type),
        }
    }

    pub fn is_sized(&self) -> bool {
        self.void_type.is_sized()
    }

    pub fn get_context(&self) -> ContextRef {
        self.void_type.get_context()
    }

    pub fn ptr_type(&self, address_space: u32) -> PointerType {
        self.void_type.ptr_type(address_space)
    }

    pub fn fn_type(&self, param_types: &[&AnyType], is_var_args: bool) -> FunctionType {
        self.void_type.fn_type(param_types, is_var_args)
    }

    pub fn void_type() -> Self {
        let void_type = unsafe {
            LLVMVoidType()
        };

        VoidType::new(void_type)
    }

    pub fn print_to_string(&self) -> &CStr {
        self.void_type.print_to_string()
    }

    pub fn print_to_stderr(&self) {
        self.void_type.print_to_stderr()
    }

    pub fn const_null_ptr(&self) -> PointerValue {
        self.void_type.const_null_ptr()
    }
}

impl AsTypeRef for VoidType {
    fn as_type_ref(&self) -> LLVMTypeRef {
        self.void_type.type_
    }
}

#[derive(Debug)]
pub struct PointerType {
    ptr_type: Type,
}

impl PointerType {
    pub(crate) fn new(ptr_type: LLVMTypeRef) -> Self {
        assert!(!ptr_type.is_null());

        PointerType {
            ptr_type: Type::new(ptr_type),
        }
    }

    pub fn is_sized(&self) -> bool {
        self.ptr_type.is_sized()
    }

    pub fn ptr_type(&self, address_space: u32) -> PointerType {
        self.ptr_type.ptr_type(address_space)
    }

    pub fn get_context(&self) -> ContextRef {
        self.ptr_type.get_context()
    }

    pub fn fn_type(&self, param_types: &[&AnyType], is_var_args: bool) -> FunctionType {
        self.ptr_type.fn_type(param_types, is_var_args)
    }

    pub fn array_type(&self, size: u32) -> ArrayType {
        self.ptr_type.array_type(size)
    }

    pub fn get_address_space(&self) -> u32 {
        unsafe {
            LLVMGetPointerAddressSpace(self.as_type_ref())
        }
    }

    pub fn print_to_string(&self) -> &CStr {
        self.ptr_type.print_to_string()
    }

    pub fn print_to_stderr(&self) {
        self.ptr_type.print_to_stderr()
    }

    pub fn const_null_ptr(&self) -> PointerValue {
        self.ptr_type.const_null_ptr()
    }

    pub fn const_null(&self) -> PointerValue {
        let null = unsafe {
            LLVMConstNull(self.as_type_ref())
        };

        PointerValue::new(null)
    }
}

impl AsTypeRef for PointerType {
    fn as_type_ref(&self) -> LLVMTypeRef {
        self.ptr_type.type_
    }
}

#[derive(Debug)]
pub struct ArrayType {
    array_type: Type,
}

impl ArrayType {
    pub(crate) fn new(array_type: LLVMTypeRef) -> Self {
        assert!(!array_type.is_null());

        ArrayType {
            array_type: Type::new(array_type),
        }
    }

    pub fn is_sized(&self) -> bool {
        self.array_type.is_sized()
    }

    pub fn ptr_type(&self, address_space: u32) -> PointerType {
        self.array_type.ptr_type(address_space)
    }

    pub fn get_context(&self) -> ContextRef {
        self.array_type.get_context()
    }

    pub fn fn_type(&self, param_types: &[&AnyType], is_var_args: bool) -> FunctionType {
        self.array_type.fn_type(param_types, is_var_args)
    }

    pub fn array_type(&self, size: u32) -> ArrayType {
        self.array_type.array_type(size)
    }

    pub fn const_array<V: BasicValue>(&self, values: &[&V]) -> ArrayValue {
        let mut values: Vec<LLVMValueRef> = values.iter()
                                                  .map(|val| val.as_value_ref())
                                                  .collect();
        let value = unsafe {
            LLVMConstArray(self.as_type_ref(), values.as_mut_ptr(), values.len() as u32)
        };

        ArrayValue::new(value)
    }

    pub fn const_null_ptr(&self) -> PointerValue {
        self.array_type.const_null_ptr()
    }

    pub fn const_null(&self) -> ArrayValue {
        let null = unsafe {
            LLVMConstNull(self.as_type_ref())
        };

        ArrayValue::new(null)
    }

    pub fn len(&self) -> u32 {
        unsafe {
            LLVMGetArrayLength(self.as_type_ref())
        }
    }

    pub fn print_to_string(&self) -> &CStr {
        self.array_type.print_to_string()
    }

    pub fn print_to_stderr(&self) {
        self.array_type.print_to_stderr()
    }
}

impl AsTypeRef for ArrayType {
    fn as_type_ref(&self) -> LLVMTypeRef {
        self.array_type.type_
    }
}

#[derive(Debug)]
// REVIEW: vec_type() is impl for IntType & FloatType. Need to
// find out if it is valid for other types too. Maybe PointerType?
pub struct VectorType {
    vec_type: Type,
}

impl VectorType {
    pub(crate) fn new(vector_type: LLVMTypeRef) -> Self {
        assert!(vector_type.is_null());

        VectorType {
            vec_type: Type::new(vector_type),
        }
    }

    pub fn size(&self) -> u32 {
        unsafe {
            LLVMGetVectorSize(self.as_type_ref())
        }
    }

    // REVIEW:
    // TypeSafety v2 (GH Issue #8) could help here by constraining
    // sub-types to be the same across the board. For now, we could
    // have V just be the set of Int & Float and any others that
    // are valid for Vectors
    // REVIEW: Maybe we could make this use &self if the vector size
    // is stored as a const and the input values took a const size?
    // Something like: values: &[&V; self.size]. Doesn't sound possible though
    pub fn const_vector<V: BasicValue>(values: &[&V]) -> VectorValue {
        let mut values: Vec<LLVMValueRef> = values.iter()
                                                  .map(|val| val.as_value_ref())
                                                  .collect();
        let vec_value = unsafe {
            LLVMConstVector(values.as_mut_ptr(), values.len() as u32)
        };

        VectorValue::new(vec_value)
    }

    pub fn const_null_ptr(&self) -> PointerValue {
        self.vec_type.const_null_ptr()
    }

    pub fn const_null(&self) -> VectorValue {
        let null = unsafe {
            LLVMConstNull(self.as_type_ref())
        };

        VectorValue::new(null)
    }

    pub fn print_to_string(&self) -> &CStr {
        self.vec_type.print_to_string()
    }

    pub fn print_to_stderr(&self) {
        self.vec_type.print_to_stderr()
    }
}

impl AsTypeRef for VectorType {
    fn as_type_ref(&self) -> LLVMTypeRef {
        self.vec_type.type_
    }
}

macro_rules! trait_type_set {
    ($trait_name:ident: $($args:ident),*) => (
        pub trait $trait_name: AsTypeRef {}

        $(
            impl $trait_name for $args {}
        )*
    );
}

macro_rules! enum_type_set {
    ($enum_name:ident: $($args:ident),*) => (
        #[derive(Debug)]
        pub enum $enum_name {
            $(
                $args($args),
            )*
        }

        impl AsTypeRef for $enum_name {
            fn as_type_ref(&self) -> LLVMTypeRef {
                match *self {
                    $(
                        $enum_name::$args(ref t) => t.as_type_ref(),
                    )*
                }
            }
        }

        $(
            impl From<$args> for $enum_name {
                fn from(value: $args) -> $enum_name {
                    $enum_name::$args(value)
                }
            }
        )*
    );
}

enum_type_set! {AnyTypeEnum: IntType, FunctionType, FloatType, PointerType, StructType, ArrayType, VoidType, VectorType}
enum_type_set! {BasicTypeEnum: IntType, FloatType, PointerType, StructType, ArrayType, VectorType}

// TODO: Possibly rename to AnyTypeTrait, BasicTypeTrait
trait_type_set! {AnyType: AnyTypeEnum, BasicTypeEnum, IntType, FunctionType, FloatType, PointerType, StructType, ArrayType, VoidType, VectorType}
trait_type_set! {BasicType: BasicTypeEnum, IntType, FloatType, PointerType, StructType, ArrayType, VectorType}

impl AnyTypeEnum {
    pub(crate) fn new(type_: LLVMTypeRef) -> AnyTypeEnum {
        let type_kind = unsafe {
            LLVMGetTypeKind(type_)
        };

        match type_kind {
            LLVMTypeKind::LLVMVoidTypeKind => AnyTypeEnum::VoidType(VoidType::new(type_)),
            LLVMTypeKind::LLVMHalfTypeKind => AnyTypeEnum::FloatType(FloatType::new(type_)),
            LLVMTypeKind::LLVMFloatTypeKind => AnyTypeEnum::FloatType(FloatType::new(type_)),
            LLVMTypeKind::LLVMDoubleTypeKind => AnyTypeEnum::FloatType(FloatType::new(type_)),
            LLVMTypeKind::LLVMX86_FP80TypeKind => AnyTypeEnum::FloatType(FloatType::new(type_)),
            LLVMTypeKind::LLVMFP128TypeKind => AnyTypeEnum::FloatType(FloatType::new(type_)),
            LLVMTypeKind::LLVMPPC_FP128TypeKind => AnyTypeEnum::FloatType(FloatType::new(type_)),
            LLVMTypeKind::LLVMLabelTypeKind => panic!("FIXME: Unsupported type: Label"),
            LLVMTypeKind::LLVMIntegerTypeKind => AnyTypeEnum::IntType(IntType::new(type_)),
            LLVMTypeKind::LLVMFunctionTypeKind => AnyTypeEnum::FunctionType(FunctionType::new(type_)),
            LLVMTypeKind::LLVMStructTypeKind => AnyTypeEnum::StructType(StructType::new(type_)),
            LLVMTypeKind::LLVMArrayTypeKind => AnyTypeEnum::ArrayType(ArrayType::new(type_)),
            LLVMTypeKind::LLVMPointerTypeKind => AnyTypeEnum::PointerType(PointerType::new(type_)),
            LLVMTypeKind::LLVMVectorTypeKind => AnyTypeEnum::VectorType(VectorType::new(type_)),
            LLVMTypeKind::LLVMMetadataTypeKind => panic!("FIXME: Unsupported type: Metadata"),
            LLVMTypeKind::LLVMX86_MMXTypeKind => panic!("FIXME: Unsupported type: MMX"),
            // LLVMTypeKind::LLVMTokenTypeKind => panic!("FIXME: Unsupported type: Token"), // Different version?
        }
    }
}

impl BasicTypeEnum {
    pub(crate) fn new(type_: LLVMTypeRef) -> BasicTypeEnum {
        let type_kind = unsafe {
            LLVMGetTypeKind(type_)
        };

        match type_kind {
            LLVMTypeKind::LLVMHalfTypeKind |
            LLVMTypeKind::LLVMFloatTypeKind |
            LLVMTypeKind::LLVMDoubleTypeKind |
            LLVMTypeKind::LLVMX86_FP80TypeKind |
            LLVMTypeKind::LLVMFP128TypeKind |
            LLVMTypeKind::LLVMPPC_FP128TypeKind => BasicTypeEnum::FloatType(FloatType::new(type_)),
            LLVMTypeKind::LLVMIntegerTypeKind => BasicTypeEnum::IntType(IntType::new(type_)),
            LLVMTypeKind::LLVMStructTypeKind => BasicTypeEnum::StructType(StructType::new(type_)),
            LLVMTypeKind::LLVMPointerTypeKind => BasicTypeEnum::PointerType(PointerType::new(type_)),
            LLVMTypeKind::LLVMArrayTypeKind => BasicTypeEnum::ArrayType(ArrayType::new(type_)),
            LLVMTypeKind::LLVMVectorTypeKind => BasicTypeEnum::VectorType(VectorType::new(type_)),
            _ => unreachable!("Unsupported type"),
        }
    }

    pub fn into_int_type(self) -> IntType {
        if let BasicTypeEnum::IntType(i) = self {
            i
        } else {
            panic!("Called BasicValueEnum.into_int_type on {:?}", self);
        }
    }

    pub fn into_float_type(self) -> FloatType {
        if let BasicTypeEnum::FloatType(f) = self {
            f
        } else {
            panic!("Called BasicValueEnum.into_float_type on {:?}", self);
        }
    }

    pub fn into_pointer_type(self) -> PointerType {
        if let BasicTypeEnum::PointerType(p) = self {
            p
        } else {
            panic!("Called BasicValueEnum.into_ptr_type on {:?}", self);
        }
    }

    pub fn into_struct_type(self) -> StructType {
        if let BasicTypeEnum::StructType(s) = self {
            s
        } else {
            panic!("Called BasicValueEnum.into_struct_type on {:?}", self);
        }
    }

    pub fn into_array_type(self) -> ArrayType {
        if let BasicTypeEnum::ArrayType(a) = self {
            a
        } else {
            panic!("Called BasicValueEnum.into_array_type on {:?}", self);
        }
    }

    pub fn into_vector_type(self) -> VectorType {
        if let BasicTypeEnum::VectorType(v) = self {
            v
        } else {
            panic!("Called BasicValueEnum.into_vector_type on {:?}", self);
        }
    }

    pub fn as_int_type(&self) -> &IntType {
        if let BasicTypeEnum::IntType(ref i) = *self {
            i
        } else {
            panic!("Called BasicValueEnum.as_int_type on {:?}", self);
        }
    }

    pub fn as_float_type(&self) -> &FloatType {
        if let BasicTypeEnum::FloatType(ref f) = *self {
            f
        } else {
            panic!("Called BasicValueEnum.as_float_type on {:?}", self);
        }
    }

    pub fn as_pointer_type(&self) -> &PointerType {
        if let BasicTypeEnum::PointerType(ref p) = *self {
            p
        } else {
            panic!("Called BasicValueEnum.as_pointer_type on {:?}", self);
        }
    }

    pub fn as_struct_type(&self) -> &StructType {
        if let BasicTypeEnum::StructType(ref s) = *self {
            s
        } else {
            panic!("Called BasicValueEnum.as_struct_type on {:?}", self);
        }
    }

    pub fn as_array_type(&self) -> &ArrayType {
        if let BasicTypeEnum::ArrayType(ref a) = *self {
            a
        } else {
            panic!("Called BasicValueEnum.as_array_type on {:?}", self);
        }
    }

    pub fn as_vector_type(&self) -> &VectorType {
        if let BasicTypeEnum::VectorType(ref v) = *self {
            v
        } else {
            panic!("Called BasicValueEnum.as_array_type on {:?}", self);
        }
    }

    pub fn is_int_type(&self) -> bool {
        if let BasicTypeEnum::IntType(_) = *self {
            true
        } else {
            false
        }
    }

    pub fn is_float_type(&self) -> bool {
        if let BasicTypeEnum::FloatType(_) = *self {
            true
        } else {
            false
        }
    }

    pub fn is_pointer_type(&self) -> bool {
        if let BasicTypeEnum::PointerType(_) = *self {
            true
        } else {
            false
        }
    }

    pub fn is_struct_type(&self) -> bool {
        if let BasicTypeEnum::StructType(_) = *self {
            true
        } else {
            false
        }
    }

    pub fn is_array_type(&self) -> bool {
        if let BasicTypeEnum::ArrayType(_) = *self {
            true
        } else {
            false
        }
    }

    pub fn is_vector_type(&self) -> bool {
        if let BasicTypeEnum::VectorType(_) = *self {
            true
        } else {
            false
        }
    }
}

// REVIEW: Possible to impl Debug for AnyType?

#[test]
fn test_function_type() {
    let context = Context::create();
    let int = context.i8_type();
    let float = context.f32_type();
    let fn_type = int.fn_type(&[&int, &int, &float], false);

    assert!(!fn_type.is_var_arg());

    let param_types = fn_type.get_param_types();

    assert_eq!(param_types.len(), 3);
    assert_eq!(param_types[0].as_int_type().as_type_ref(), int.as_type_ref());
    assert_eq!(param_types[1].as_int_type().as_type_ref(), int.as_type_ref());
    assert_eq!(param_types[2].as_float_type().as_type_ref(), float.as_type_ref());

    let fn_type = int.fn_type(&[&int, &float], true);

    assert!(fn_type.is_var_arg());
}