use common::vec::*;

//A FIFO Queue
pub struct Queue<T> {
    vec: Vec<T>
}

impl<T> Queue<T> {
    pub fn new() -> Queue<T> {
        Queue {
            vec: Vec::new()
        }
    }

    pub fn push(&mut self, value: T){
        self.vec.push(value);
    }

    pub fn pop(&mut self) -> Option<T> {
        return self.vec.remove(0);
    }
}
