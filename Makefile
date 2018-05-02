build:
	cargo build

haproxy-run:
	docker run --rm --name haproxy-alert-haproxy --network="host" -e SYSLOGD=1 -v $(PWD)/haproxy:/usr/local/etc/haproxy:ro haproxy:alpine

haproxy-restart:
	docker kill -s HUP haproxy-alert-haproxy