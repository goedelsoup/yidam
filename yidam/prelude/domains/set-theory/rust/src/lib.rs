use std::collections::BTreeSet;

pub fn union(a: &[i64], b: &[i64]) -> Vec<i64> {
    a.iter().chain(b.iter()).copied().collect::<BTreeSet<_>>().into_iter().collect()
}

pub fn intersection(a: &[i64], b: &[i64]) -> Vec<i64> {
    let bs: BTreeSet<i64> = b.iter().copied().collect();
    a.iter().copied().filter(|x| bs.contains(x)).collect::<BTreeSet<_>>().into_iter().collect()
}

pub fn difference(a: &[i64], b: &[i64]) -> Vec<i64> {
    let bs: BTreeSet<i64> = b.iter().copied().collect();
    a.iter().copied().filter(|x| !bs.contains(x)).collect::<BTreeSet<_>>().into_iter().collect()
}

pub fn is_subset(a: &[i64], b: &[i64]) -> bool {
    let bs: BTreeSet<i64> = b.iter().copied().collect();
    a.iter().all(|x| bs.contains(x))
}
