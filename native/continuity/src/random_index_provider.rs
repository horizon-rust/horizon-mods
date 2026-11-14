//! 1:1 of me.pepperbell.continuity.client.util.RandomIndexProvider.

use crate::math_util::remove_sign_bit;

/// java: public interface RandomIndexProvider.
pub trait RandomIndexProvider {
    /// java: int getRandomIndex(int random).
    fn get_random_index(&self, random: i32) -> i32;
}

/// java: class Unweighted implements RandomIndexProvider.
pub struct Unweighted {
    size: i32,
}

impl Unweighted {
    /// java: Unweighted(int size).
    pub fn new(size: i32) -> Unweighted {
        Unweighted { size }
    }
}

impl RandomIndexProvider for Unweighted {
    fn get_random_index(&self, random: i32) -> i32 {
        // java: MathUtil.removeSignBit(random) % size.
        remove_sign_bit(random) % self.size
    }
}

/// java: interface Factory.
pub trait RandomIndexProviderFactory {
    /// java: RandomIndexProvider createIndexProvider(int size).
    fn create_index_provider(&self, size: i32) -> Box<dyn RandomIndexProvider>;
}

/// java: class UnweightedFactory implements Factory.
pub struct UnweightedFactory;

impl UnweightedFactory {
    /// java: public static final UnweightedFactory INSTANCE.
    pub const INSTANCE: UnweightedFactory = UnweightedFactory;
}

impl RandomIndexProviderFactory for UnweightedFactory {
    fn create_index_provider(&self, size: i32) -> Box<dyn RandomIndexProvider> {
        Box::new(Unweighted::new(size))
    }
}

/// java: class Weighted implements RandomIndexProvider.
pub struct Weighted {
    weights: Vec<i32>,
    weight_sum: i32,
    max_index: i32,
}

impl Weighted {
    /// java: Weighted(int[] weights, int weightSum).
    pub fn new(weights: Vec<i32>, weight_sum: i32) -> Weighted {
        let max_index = weights.len() as i32 - 1;
        Weighted {
            weights,
            weight_sum,
            max_index,
        }
    }
}

impl RandomIndexProvider for Weighted {
    fn get_random_index(&self, random: i32) -> i32 {
        // java: getRandomIndex.
        let mut temp_weight = remove_sign_bit(random).wrapping_rem(self.weight_sum);
        let mut index: i32 = 0;
        while index < self.max_index && temp_weight >= self.weights[index as usize] {
            temp_weight -= self.weights[index as usize];
            index += 1;
        }
        index
    }
}

/// java: class WeightedFactory implements Factory.
pub struct WeightedFactory {
    weights: Vec<i32>,
}

impl WeightedFactory {
    /// java: WeightedFactory(int[] weights).
    pub fn new(weights: Vec<i32>) -> WeightedFactory {
        WeightedFactory { weights }
    }
}

impl RandomIndexProviderFactory for WeightedFactory {
    fn create_index_provider(&self, size: i32) -> Box<dyn RandomIndexProvider> {
        // java: createIndexProvider - copy into a size-length array, pad with the average weight.
        let new_len = size.max(0) as usize;
        let mut new_weights = vec![0i32; new_len];
        let copied_length = self.weights.len().min(new_weights.len());
        new_weights[..copied_length].copy_from_slice(&self.weights[..copied_length]);

        let mut weight_sum = 0i32;
        for i in 0..copied_length {
            weight_sum = weight_sum.wrapping_add(new_weights[i]);
        }

        if copied_length < new_weights.len() {
            // java: averageWeight = weightSum / copiedLength (copiedLength is nonzero here in practice;
            // a zero-division keeps Java's ArithmeticException as a panic).
            let average_weight = weight_sum / copied_length as i32;
            for i in copied_length..new_weights.len() {
                new_weights[i] = average_weight;
                weight_sum = weight_sum.wrapping_add(average_weight);
            }
        }

        Box::new(Weighted::new(new_weights, weight_sum))
    }
}
