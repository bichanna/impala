pub mod object;
pub mod value;

use std::collections::HashMap;
use std::ptr;

use byteorder::{ByteOrder, LittleEndian};

use self::object::ObjectType;
use self::value::Value;
use crate::compiler::opcode::{OpCode, Position};

// Constants
const STACK_MAX: usize = 256;

macro_rules! read_byte {
    ($self: expr) => {{
        $self.ip = unsafe { $self.ip.add(1) };
        unsafe { *$self.ip }
    }};
}

macro_rules! push_or_err {
    ($self: expr, $value: expr) => {
        match $value {
            Ok(v) => $self.push(v),
            Err(_msg) => {
                // TODO: Report error
            }
        }
    };
}

macro_rules! binary_op {
    ($self: expr, $op: tt) => {
        let right = $self.pop();
        let left = $self.pop();
        push_or_err!($self, left $op right);
    };
}

pub struct VM<'a> {
    /// The bytecode to be run
    bytecode: &'a Vec<u8>,
    /// The constants pool, holds all constants in a program
    values: &'a Vec<Value>,
    /// Position information only used when runtime errors occur
    positions: &'a HashMap<usize, Position>,
    /// The file name of the source code
    filename: &'a str,
    /// Source code
    source: &'a String,
    /// Instruction pointer, holds the current instruction being executed
    ip: *const u8,
    /// This stack can be safely accessed without bound checking
    stack: Box<[Value; STACK_MAX]>,
    stack_top: *mut Value,
    /// All global variables
    globals: HashMap<String, Value>,
}

impl<'a> VM<'a> {
    pub fn new(
        filename: &'a str,
        source: &'a String,
        bytecode: &'a Vec<u8>,
        values: &'a Vec<Value>,
        positions: &'a HashMap<usize, Position>,
    ) -> Self {
        Self {
            bytecode,
            values,
            positions,
            ip: bytecode.as_ptr(),
            filename,
            source,
            stack: Box::new([Value::Null; STACK_MAX]),
            stack_top: ptr::null_mut(),
            globals: HashMap::new(),
        }
    }

    /// The heart of the VM
    pub fn run(&mut self) {
        let mut instruction = OpCode::u8_to_opcode(unsafe { *self.ip }).unwrap();
        self.stack_top = &mut self.stack[0] as *mut Value;

        loop {
            match instruction {
                OpCode::Return => {
                    println!("{}", self.pop().print());
                    break;
                }
                OpCode::Constant => {
                    let value = self.read_constant(false);
                    self.push(value);
                }
                OpCode::ConstantLong => {
                    let value = self.read_constant(true);
                    self.push(value);
                }
                OpCode::Negate => {
                    push_or_err!(self, -self.pop());
                }
                OpCode::Add => {
                    binary_op!(self, +);
                }
                OpCode::Sub => {
                    binary_op!(self, -);
                }
                OpCode::Mult => {
                    binary_op!(self, *);
                }
                OpCode::Div => {
                    binary_op!(self, /);
                }
                OpCode::Mod => {
                    binary_op!(self, %);
                }
                OpCode::DefineGlobalVar => {
                    self.set_global(true);
                }
                OpCode::GetGlobalVar => {
                    self.get_global_var();
                }
                OpCode::SetGlobalVar => {
                    self.set_global(false);
                }
                OpCode::Pop => {
                    self.pop();
                }
                OpCode::SetLocalVar => {
                    let slot = read_byte!(self);
                    self.stack[slot as usize] = unsafe { *self.peek(0) };
                }
                OpCode::GetLocalVar => {
                    let slot = read_byte!(self);
                    self.push(self.stack[slot as usize]);
                }
            }

            instruction = OpCode::u8_to_opcode(read_byte!(self)).unwrap();
        }

        // rudementary garbage collection
        for value in self.values {
            match value {
                Value::Object(obj) => obj.free(),
                _ => {}
            }
        }
    }

    /// Gets global variable and pushes it to the stack
    fn get_global_var(&mut self) {
        let value = self.pop();
        match value {
            Value::Object(obj) => match obj.obj_type {
                ObjectType::Identifier => {
                    let variable_name = unsafe { (*(*obj.obj).string).clone() };
                    match self.globals.get(&variable_name) {
                        Some(value) => self.push(*value),
                        None => {
                            // TODO: report error
                        }
                    }
                }
                _ => {
                    // TODO: report error
                }
            },
            _ => {
                // TODO: report error
            }
        }
    }

    /// Defines globals
    fn set_global(&mut self, set: bool) {
        let right = self.pop();
        let left = self.pop();
        match left {
            Value::Object(left_obj) => match left_obj.obj_type {
                ObjectType::Identifier => {
                    let key = unsafe { (*(*left_obj.obj).string).to_owned() };
                    if !set {
                        // used for plain variable
                        // TODO: check for variables already defined and return error
                        self.globals.insert(key, right);
                    } else {
                        let result = self.globals.contains_key(&key);
                        if result {
                            self.globals.insert(key, right);
                        } else {
                            // TODO: report error
                        }
                    }
                    self.push(right);
                }
                ObjectType::List => match right {
                    Value::Object(right_obj) => match right_obj.obj_type {
                        ObjectType::List => {
                            let left = unsafe { (*(*left_obj.obj).list).clone() };
                            let right = unsafe { (*(*right_obj.obj).list).clone() };

                            // check if both of them are the same length
                            if left.len() != right.len() {
                                // TODO: report error
                            }

                            for (i, id) in left.into_iter().enumerate() {
                                match *id {
                                    Value::Empty => continue,
                                    Value::Object(obj) => match obj.obj_type {
                                        ObjectType::Atom => {
                                            let key =
                                                unsafe { (*(*left_obj.obj).string).to_owned() };
                                            if !set {
                                                // used for plain variable
                                                // TODO: check for variables already defined and return error
                                                self.globals.insert(key, *right[i]);
                                            } else {
                                                let result = self.globals.contains_key(&key);
                                                if result {
                                                    self.globals.insert(key, *right[i]);
                                                } else {
                                                    // TODO: report error
                                                }
                                            }
                                            self.push(*right[i]);
                                        }
                                        _ => {
                                            // TODO: report error
                                        }
                                    },
                                    _ => {
                                        // TODO: report error
                                    }
                                }
                            }
                        }
                        _ => {
                            // TODO: report error
                        }
                    },
                    _ => {
                        // TODO: report error
                    }
                },
                ObjectType::Object => match right {
                    Value::Object(right_obj) => match right_obj.obj_type {
                        ObjectType::Object => {
                            let left = unsafe { (*(*left_obj.obj).object).clone() };
                            let right = unsafe { (*(*right_obj.obj).object).clone() };

                            for (key, value) in left.into_iter() {
                                if key == "_" {
                                    continue;
                                }

                                match &*value {
                                    Value::Object(obj) => match obj.obj_type {
                                        ObjectType::Identifier => {
                                            match right.get(&key) {
                                                Some(to_be_assigned) => {
                                                    let key = unsafe {
                                                        (*(*left_obj.obj).string).to_owned()
                                                    };
                                                    if !set {
                                                        // used for plain variable
                                                        // TODO: check for variables already defined and return error
                                                        self.globals.insert(key, **to_be_assigned);
                                                    } else {
                                                        let result =
                                                            self.globals.contains_key(&key);
                                                        if result {
                                                            self.globals
                                                                .insert(key, **to_be_assigned);
                                                        } else {
                                                            // TODO: report error
                                                        }
                                                    }
                                                    self.push(**to_be_assigned);
                                                }
                                                None => {
                                                    // TODO: report error
                                                }
                                            }
                                        }
                                        _ => {
                                            // TODO: report error
                                        }
                                    },
                                    _ => {
                                        // TODO: report error
                                    }
                                }
                            }
                        }
                        _ => {
                            // TODO: report error
                        }
                    },
                    _ => {
                        // TODO: report error
                    }
                },
                _ => {
                    // can't happen, nothing happens
                }
            },
            _ => {
                // TODO: report error
            }
        }
    }

    /// Pushes a Value onto the stack
    fn push(&mut self, value: Value) {
        unsafe { *self.stack_top = value }
        self.stack_top = unsafe { self.stack_top.add(1) };
    }

    /// Pops a Value from the stack
    fn pop(&mut self) -> Value {
        self.stack_top = unsafe { self.stack_top.sub(1) };
        unsafe { *self.stack_top }
    }

    /// Reads a Value and returns it
    fn read_constant(&mut self, long: bool) -> Value {
        if long {
            let bytes = [read_byte!(self), read_byte!(self)];
            let constant = LittleEndian::read_u16(&bytes) as usize;
            self.values[constant]
        } else {
            self.values[read_byte!(self) as usize]
        }
    }

    /// Peeks a [`Value`] from the stack
    fn peek(&mut self, n: usize) -> *mut Value {
        unsafe { self.stack_top.sub(n + 1) }
    }
}

// Tests
#[cfg(test)]
mod tests {
    use super::{object::ObjectUnion, *};

    #[test]
    fn test_binary() {
        let bytecode: Vec<u8> = vec![1, 0, 1, 1, 4, 0];
        let values: Vec<Value> = vec![Value::Int(1), Value::Int(1)];
        let positions = HashMap::new();
        let source = "1 + 1".to_string();

        let mut vm = VM::new("input", &source, &bytecode, &values, &positions);
        vm.run();

        assert_eq!(unsafe { *vm.stack_top }, Value::Int(2));
    }

    #[test]
    fn test_unary() {
        let bytecode: Vec<u8> = vec![1, 0, 3, 0];
        let values: Vec<Value> = vec![Value::Bool(false)];
        let positions = HashMap::new();
        let source = "not false".to_string();

        let mut vm = VM::new("input", &source, &bytecode, &values, &positions);
        vm.run();

        assert_eq!(unsafe { *vm.stack_top }, Value::Bool(true));
    }

    #[test]
    fn test_global() {
        let bytecode: Vec<u8> = vec![1, 0, 1, 1, 9, 1, 2, 1, 3, 11, 0];
        let values: Vec<Value> = vec![
            Value::Object(object::Object {
                obj_type: ObjectType::Identifier,
                obj: &mut ObjectUnion {
                    string: &mut "Hello".to_string() as *mut String,
                } as *mut ObjectUnion,
            }),
            Value::Int(5),
            Value::Object(object::Object {
                obj_type: ObjectType::Identifier,
                obj: &mut ObjectUnion {
                    string: &mut "Hello".to_string() as *mut String,
                } as *mut ObjectUnion,
            }),
            Value::Bool(false),
        ];
        let positions = HashMap::new();
        let source = r#"Hello := 5 Hello = false"#.to_string();

        let mut vm = VM::new("input", &source, &bytecode, &values, &positions);
        vm.run();

        assert_eq!(unsafe { *vm.stack_top }, Value::Bool(false));
    }
}
