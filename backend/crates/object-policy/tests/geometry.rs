use grove_object_policy::multipart::*;

#[test]
fn geometry_covers_every_byte_at_part_boundaries() {
    for part_size in [1, 5, 64, 5 * 1024 * 1024_i64] {
        for declared in [1, part_size, part_size + 1, part_size * 3 - 1] {
            let count = part_count(declared, part_size);
            let mut total = 0;
            for number in 1..=count {
                assert_eq!(part_offset(part_size, number), total as u64);
                let size = part_expected_size(declared, part_size, number);
                assert!((1..=part_size).contains(&size));
                total += size;
            }
            assert_eq!(total, declared);
        }
    }
}

#[test]
fn maximum_part_count_has_a_full_final_part() {
    let part_size = 5 * 1024 * 1024_i64;
    let declared = part_size * i64::from(MAX_PARTS);
    assert_eq!(part_count(declared, part_size), MAX_PARTS);
    assert_eq!(
        part_expected_size(declared, part_size, MAX_PARTS),
        part_size
    );
    assert!(part_number_ok(MAX_PARTS, MAX_PARTS));
    assert!(!part_number_ok(MAX_PARTS + 1, MAX_PARTS));
}

#[test]
fn geometry_derives_from_declared_and_frozen_part_size() {
    let (declared, part) = (12 * 1024 * 1024_i64, 5 * 1024 * 1024_i64);
    assert_eq!(part_count(declared, part), 3);
    assert_eq!(part_expected_size(declared, part, 1), part);
    assert_eq!(part_expected_size(declared, part, 2), part);
    assert_eq!(part_expected_size(declared, part, 3), 2 * 1024 * 1024);
    assert_eq!(part_offset(part, 3), (10 * 1024 * 1024) as u64);
    assert_eq!(part_count(10 * 1024 * 1024, part), 2);
    assert_eq!(part_expected_size(10 * 1024 * 1024, part, 2), part);
    assert_eq!(part_count(1, part), 1);
    assert_eq!(part_expected_size(1, part, 1), 1);
}

#[test]
fn part_number_uses_the_inclusive_range() {
    assert!(!part_number_ok(0, 3));
    assert!(part_number_ok(1, 3));
    assert!(part_number_ok(3, 3));
    assert!(!part_number_ok(4, 3));
    assert!(part_number_ok(1, 1));
}
