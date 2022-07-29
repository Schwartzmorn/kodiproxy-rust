# Files library

Underlying database layer for a minimalistic WebDAV server. It allows saving files and historizes all the changes.

## Behaviour

The entry points are /files/* and /file-versions/*.

/files/* accepts the methods GET, HEAD, PUT, DELETE and MOVE.

For PUT, DELETE and MOVE, the current numerical version should be sent in the ETag header of the request. No ETag header should be sent in the PUT request if the file does not currently exists.

The version is tracked per path and persists for a given path through deletion, move, etc.

For the MOVE method, the destination should not have a file saved.
