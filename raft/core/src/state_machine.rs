// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub trait StateMachine {
    type Payload;

    fn apply(&mut self, payload: &Self::Payload);
    fn get(&self, key: &str) -> Option<&str>;
}
