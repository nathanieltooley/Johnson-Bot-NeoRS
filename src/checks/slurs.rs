static SLURS: [&str; 10] = [
    "nigger", "nigga", "negro", "chink", "niglet", "nigtard", "gook", "kike", "faggot", "beaner",
];

pub fn contains_slur(message: &str) -> bool {
    for s in SLURS {
        if message.contains(s) {
            return true;
        }
    }

    false
}
