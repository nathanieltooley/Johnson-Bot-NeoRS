rsync -avz ./target/release/johnson-nrs ${PUSH_HOST}:/usr/local/bin/johnson-nrs
ssh ${HOST} -t "systemctl --user -M ${HOST_NAME}@ restart j_nrs.service; systemctl --user -M ${HOST_NAME}@ status j_nrs.service"
