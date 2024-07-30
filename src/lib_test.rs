use crate::Chip;

const TEST_RATE: u32 = 49716;

#[test]
fn test_table_linear_rates() {
    let chip = Chip::new(TEST_RATE);
    let linear_rates = &chip.tables.linear_rates;
    assert_eq!(linear_rates[0], 2047);
    assert_eq!(linear_rates[1], 2559);
    assert_eq!(linear_rates[2], 3071);
    assert_eq!(linear_rates[3], 3583);
    assert_eq!(linear_rates[4], 4095);
    assert_eq!(linear_rates[5], 5119);
    assert_eq!(linear_rates[6], 6143);
    assert_eq!(linear_rates[7], 7167);
    assert_eq!(linear_rates[8], 8191);
    assert_eq!(linear_rates[9], 10239);
    assert_eq!(linear_rates[10], 12287);
    assert_eq!(linear_rates[11], 14335);
    assert_eq!(linear_rates[12], 16383);
    assert_eq!(linear_rates[13], 20479);
    assert_eq!(linear_rates[14], 24575);
    assert_eq!(linear_rates[15], 28671);
    assert_eq!(linear_rates[16], 32767);
    assert_eq!(linear_rates[17], 40959);
    assert_eq!(linear_rates[18], 49151);
    assert_eq!(linear_rates[19], 57343);
    assert_eq!(linear_rates[20], 65535);
    assert_eq!(linear_rates[21], 81919);
    assert_eq!(linear_rates[22], 98303);
    assert_eq!(linear_rates[23], 114687);
    assert_eq!(linear_rates[24], 131071);
    assert_eq!(linear_rates[25], 163839);
    assert_eq!(linear_rates[26], 196607);
    assert_eq!(linear_rates[27], 229375);
    assert_eq!(linear_rates[28], 262143);
    assert_eq!(linear_rates[29], 327679);
    assert_eq!(linear_rates[30], 393215);
    assert_eq!(linear_rates[31], 458751);
    assert_eq!(linear_rates[32], 524286);
    assert_eq!(linear_rates[33], 655358);
    assert_eq!(linear_rates[34], 786430);
    assert_eq!(linear_rates[35], 917502);
    assert_eq!(linear_rates[36], 1048573);
    assert_eq!(linear_rates[37], 1310717);
    assert_eq!(linear_rates[38], 1572860);
    assert_eq!(linear_rates[39], 1835004);
    assert_eq!(linear_rates[40], 2097147);
    assert_eq!(linear_rates[41], 2621434);
    assert_eq!(linear_rates[42], 3145721);
    assert_eq!(linear_rates[43], 3670008);
    assert_eq!(linear_rates[44], 4194295);
    assert_eq!(linear_rates[45], 5242869);
    assert_eq!(linear_rates[46], 6291443);
    assert_eq!(linear_rates[47], 7340017);
    assert_eq!(linear_rates[48], 8388591);
    assert_eq!(linear_rates[49], 10485739);
    assert_eq!(linear_rates[50], 12582887);
    assert_eq!(linear_rates[51], 14680035);
    assert_eq!(linear_rates[52], 16777183);
    assert_eq!(linear_rates[53], 20971478);
    assert_eq!(linear_rates[54], 25165774);
    assert_eq!(linear_rates[55], 29360070);
    assert_eq!(linear_rates[56], 33554366);
    assert_eq!(linear_rates[57], 41942957);
    assert_eq!(linear_rates[58], 50331549);
    assert_eq!(linear_rates[59], 58720141);
    assert_eq!(linear_rates[60], 67108732);
    assert_eq!(linear_rates[61], 67108732);
    assert_eq!(linear_rates[62], 67108732);
    assert_eq!(linear_rates[63], 67108732);
    assert_eq!(linear_rates[64], 67108732);
    assert_eq!(linear_rates[65], 67108732);
    assert_eq!(linear_rates[66], 67108732);
    assert_eq!(linear_rates[67], 67108732);
    assert_eq!(linear_rates[68], 67108732);
    assert_eq!(linear_rates[69], 67108732);
    assert_eq!(linear_rates[70], 67108732);
    assert_eq!(linear_rates[71], 67108732);
    assert_eq!(linear_rates[72], 67108732);
    assert_eq!(linear_rates[73], 67108732);
    assert_eq!(linear_rates[74], 67108732);
    assert_eq!(linear_rates[75], 67108732);
}

#[test]
fn test_table_attack_rates() {
    let chip = Chip::new(TEST_RATE);
    let attack_rates = &chip.tables.attack_rates;
    assert_eq!(attack_rates[0], 2078);
    assert_eq!(attack_rates[1], 2607);
    assert_eq!(attack_rates[2], 3117);
    assert_eq!(attack_rates[3], 3584);
    assert_eq!(attack_rates[4], 4155);
    assert_eq!(attack_rates[5], 5213);
    assert_eq!(attack_rates[6], 6233);
    assert_eq!(attack_rates[7], 7168);
    assert_eq!(attack_rates[8], 8311);
    assert_eq!(attack_rates[9], 10426);
    assert_eq!(attack_rates[10], 12466);
    assert_eq!(attack_rates[11], 14336);
    assert_eq!(attack_rates[12], 16621);
    assert_eq!(attack_rates[13], 20853);
    assert_eq!(attack_rates[14], 24933);
    assert_eq!(attack_rates[15], 28672);
    assert_eq!(attack_rates[16], 33243);
    assert_eq!(attack_rates[17], 41705);
    assert_eq!(attack_rates[18], 49865);
    assert_eq!(attack_rates[19], 57344);
    assert_eq!(attack_rates[20], 66486);
    assert_eq!(attack_rates[21], 83419);
    assert_eq!(attack_rates[22], 99729);
    assert_eq!(attack_rates[23], 114688);
    assert_eq!(attack_rates[24], 132991);
    assert_eq!(attack_rates[25], 166839);
    assert_eq!(attack_rates[26], 199488);
    assert_eq!(attack_rates[27], 229431);
    assert_eq!(attack_rates[28], 266047);
    assert_eq!(attack_rates[29], 333759);
    assert_eq!(attack_rates[30], 399071);
    assert_eq!(attack_rates[31], 459087);
    assert_eq!(attack_rates[32], 532350);
    assert_eq!(attack_rates[33], 667998);
    assert_eq!(attack_rates[34], 798142);
    assert_eq!(attack_rates[35], 918846);
    assert_eq!(attack_rates[36], 1065469);
    assert_eq!(attack_rates[37], 1337277);
    assert_eq!(attack_rates[38], 1598204);
    assert_eq!(attack_rates[39], 1840380);
    assert_eq!(attack_rates[40], 2135035);
    assert_eq!(attack_rates[41], 2680954);
    assert_eq!(attack_rates[42], 3196409);
    assert_eq!(attack_rates[43], 3692408);
    assert_eq!(attack_rates[44], 4285431);
    assert_eq!(attack_rates[45], 5384949);
    assert_eq!(attack_rates[46], 6428147);
    assert_eq!(attack_rates[47], 7431409);
    assert_eq!(attack_rates[48], 8630255);
    assert_eq!(attack_rates[49], 10864619);
    assert_eq!(attack_rates[50], 12856295);
    assert_eq!(attack_rates[51], 15045603);
    assert_eq!(attack_rates[52], 17256415);
    assert_eq!(attack_rates[53], 20971478);
    assert_eq!(attack_rates[54], 26045665);
    assert_eq!(attack_rates[55], 30822340);
    assert_eq!(attack_rates[56], 33554366);
    assert_eq!(attack_rates[57], 41942957);
    assert_eq!(attack_rates[58], 54902677);
    assert_eq!(attack_rates[59], 58720141);
    assert_eq!(attack_rates[60], 67108732);
    assert_eq!(attack_rates[61], 67108732);
    assert_eq!(attack_rates[62], 134217728);
    assert_eq!(attack_rates[63], 134217728);
    assert_eq!(attack_rates[64], 134217728);
    assert_eq!(attack_rates[65], 134217728);
    assert_eq!(attack_rates[66], 134217728);
    assert_eq!(attack_rates[67], 134217728);
    assert_eq!(attack_rates[68], 134217728);
    assert_eq!(attack_rates[69], 134217728);
    assert_eq!(attack_rates[70], 134217728);
    assert_eq!(attack_rates[71], 134217728);
    assert_eq!(attack_rates[72], 134217728);
    assert_eq!(attack_rates[73], 134217728);
    assert_eq!(attack_rates[74], 134217728);
    assert_eq!(attack_rates[75], 134217728);
}

#[test]
fn test_freq_mul() {
    let chip = Chip::new(TEST_RATE);
    let freq_mul = &chip.tables.freq_mul;
    assert_eq!(freq_mul[0], 2048);
    assert_eq!(freq_mul[1], 4096);
    assert_eq!(freq_mul[2], 8192);
    assert_eq!(freq_mul[3], 12288);
    assert_eq!(freq_mul[4], 16384);
    assert_eq!(freq_mul[5], 20480);
    assert_eq!(freq_mul[6], 24576);
    assert_eq!(freq_mul[7], 28672);
    assert_eq!(freq_mul[8], 32768);
    assert_eq!(freq_mul[9], 36864);
    assert_eq!(freq_mul[10], 40960);
    assert_eq!(freq_mul[11], 40960);
    assert_eq!(freq_mul[12], 49152);
    assert_eq!(freq_mul[13], 49152);
    assert_eq!(freq_mul[14], 61440);
    assert_eq!(freq_mul[15], 61440);
}
