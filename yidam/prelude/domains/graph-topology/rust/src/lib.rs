fn find_root(parent: &mut Vec<usize>, mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]]; // path halving
        x = parent[x];
    }
    x
}

pub fn clustering_coefficient(degree: u32, triangle_count: u32) -> f64 {
    if degree < 2 {
        return 0.0;
    }
    let possible = degree as f64 * (degree as f64 - 1.0) / 2.0;
    triangle_count as f64 / possible
}

pub fn connected_components(node_count: u32, edges: &[[u32; 2]]) -> u32 {
    if node_count == 0 {
        return 0;
    }
    let n = node_count as usize;
    let mut parent: Vec<usize> = (0..n).collect();
    for edge in edges {
        let a = find_root(&mut parent, edge[0] as usize);
        let b = find_root(&mut parent, edge[1] as usize);
        if a != b {
            parent[a] = b;
        }
    }
    let mut count = 0u32;
    for i in 0..n {
        if find_root(&mut parent, i) == i {
            count += 1;
        }
    }
    count
}
