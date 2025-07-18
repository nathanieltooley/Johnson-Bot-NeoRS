cargo build -r
rsync -avz ./target/release/johnson-nrs ${PUSH_HOST}:/usr/local/bin/johnson-nrs
ssh ${HOST} -t "sudo systemctl restart j_nrs.service; sudo systemctl status j_nrs.service"
