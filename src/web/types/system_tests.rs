use super::*;

#[test]
fn process_info_serialize() {
    let p = ProcessInfo {
        pid: 1234,
        name: "opman".into(),
        cpu: 12.5,
        mem: 4096,
        status: "Run".into(),
        disk_read: 10,
        disk_write: 20,
    };
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v["pid"], 1234);
    assert_eq!(v["name"], "opman");
    assert_eq!(v["cpu"], 12.5);
    assert_eq!(v["mem"], 4096);
    assert_eq!(v["status"], "Run");
    assert_eq!(v["disk_read"], 10);
    assert_eq!(v["disk_write"], 20);
    assert!(format!("{:?}", p.clone()).contains("ProcessInfo"));
}

#[test]
fn disk_info_serialize() {
    let d = DiskInfo {
        name: "sda".into(),
        mount: "/".into(),
        total: 1000,
        used: 400,
        fs_type: "ext4".into(),
    };
    let v = serde_json::to_value(&d).unwrap();
    assert_eq!(v["name"], "sda");
    assert_eq!(v["mount"], "/");
    assert_eq!(v["total"], 1000);
    assert_eq!(v["used"], 400);
    assert_eq!(v["fs_type"], "ext4");
    assert!(format!("{:?}", d.clone()).contains("DiskInfo"));
}

#[test]
fn network_info_serialize() {
    let n = NetworkInfo {
        name: "eth0".into(),
        rx_bytes: 500,
        tx_bytes: 600,
    };
    let v = serde_json::to_value(&n).unwrap();
    assert_eq!(v["name"], "eth0");
    assert_eq!(v["rx_bytes"], 500);
    assert_eq!(v["tx_bytes"], 600);
    assert!(format!("{:?}", n.clone()).contains("NetworkInfo"));
}

#[test]
fn system_stats_serialize_full() {
    let s = SystemStats {
        mem_total: 8192,
        mem_used: 4096,
        swap_total: 2048,
        swap_used: 100,
        cpu_usage: vec![10.0, 20.0],
        cpu_avg: 15.0,
        uptime_secs: 3600,
        hostname: "host".into(),
        load_avg: [1.0, 2.0, 3.0],
        processes: vec![ProcessInfo {
            pid: 1,
            name: "init".into(),
            cpu: 0.0,
            mem: 1,
            status: "S".into(),
            disk_read: 0,
            disk_write: 0,
        }],
        process_count: 100,
        disks: vec![DiskInfo {
            name: "sda".into(),
            mount: "/".into(),
            total: 1,
            used: 0,
            fs_type: "ext4".into(),
        }],
        networks: vec![NetworkInfo {
            name: "lo".into(),
            rx_bytes: 0,
            tx_bytes: 0,
        }],
    };
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["mem_total"], 8192);
    assert_eq!(v["mem_used"], 4096);
    assert_eq!(v["swap_total"], 2048);
    assert_eq!(v["swap_used"], 100);
    assert_eq!(v["cpu_usage"][1], 20.0);
    assert_eq!(v["cpu_avg"], 15.0);
    assert_eq!(v["uptime_secs"], 3600);
    assert_eq!(v["hostname"], "host");
    assert_eq!(v["load_avg"][2], 3.0);
    assert_eq!(v["processes"][0]["pid"], 1);
    assert_eq!(v["process_count"], 100);
    assert_eq!(v["disks"][0]["name"], "sda");
    assert_eq!(v["networks"][0]["name"], "lo");
    assert!(format!("{:?}", s.clone()).contains("SystemStats"));
}
