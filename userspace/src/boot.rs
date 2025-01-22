#![no_std]
#![no_main]
use userspace::*;

#[unsafe(no_mangle)]
extern "C" fn _start() {
    let mut buf = [0u8; 1024];
    printf("TOTALLY A TERMINAL\n", 0, 0);
    loop {
        printf("\n$ ",0, 0);
        let l = getline(&mut buf);
        printf("\n", 0, 0);
        let s = bytes_to_str(&buf, l);

        if prefix(s, "help") {
            printf("echo x -> print x\n", 0, 0);
            printf("read fno -> read file / list dir with this file no (root is 0)\n", 0, 0);
            printf("help -> show this\n", 0, 0);
            printf("exit -> shut down\n", 0, 0);
        } else if prefix(s, "echo ") {
            printf(&s[5..], 0, 0);
        } else if prefix(s, "read ") {
            match s[5..].parse::<u64>() {
                Ok(inode) => {
                    printf("Reading inode", inode, 0);
                    printf("\n\n", 0, 0);
                    let l = readi(inode, &mut buf);
                    let s = bytes_to_str(&buf, l);
                    printf(s, 0, 0);
                },
                Err(_) => {
                    printf("Bad inode no", 0, 0);
                }
            }
        } else if prefix(s, "exit") {
            break;
        } else {
            printf("Unknown command, use help to see cmds", 0, 0);
        }

        sleep(10000);
    }
}
