# Changes

## 0.5.8

* Update links to reflect repository ownership change: https://github.com/frewsxcv/tiny-http -> https://github.com/tiny-http/tiny-http

## 0.5.7

* Fix using Transfer-Encoding: identity with no content length
 * https://github.com/tiny-http/tiny-http/pull/126

## 0.5.6

* Update link to documentation
 * https://github.com/tiny-http/tiny-http/pull/123
* Fix websockets
 * https://github.com/tiny-http/tiny-http/pull/124
* Drop the request reader earlier
 * https://github.com/tiny-http/tiny-http/pull/125

## 0.5.5

* Start using the log crate
 * https://github.com/tiny-http/tiny-http/pull/121
* Unblock the accept thread on shutdown
 * https://github.com/tiny-http/tiny-http/pull/120

## 0.5.4

* Fix compilation warnings
 * https://github.com/tiny-http/tiny-http/pull/118

## 0.5.3

* Add try_recv_timeout function to the server
 * https://github.com/tiny-http/tiny-http/pull/116

## 0.5.2

* Update ascii to version 0.7
 * https://github.com/tiny-http/tiny-http/pull/114

## 0.5.1

* Request::respond now returns an IoResult
 * https://github.com/tiny-http/tiny-http/pull/110

## 0.5.0

* HTTPS support
 * https://github.com/tiny-http/tiny-http/pull/107
* Rework the server creation API
 * https://github.com/tiny-http/tiny-http/pull/106

## 0.4.1

* Allow binding to a nic by specifying the socket address
 * https://github.com/tiny-http/tiny-http/pull/103

## 0.4.0

* Make Method into an enum instead of a character string
 * https://github.com/tiny-http/tiny-http/pull/102
