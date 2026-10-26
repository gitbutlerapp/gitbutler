# Desktop

- Hold RTK queries from services (`service.thing(id)`) in a `$derived` and read
  `.result` from that. Calling the service inline in markup subscribes again on
  every read, so a query that keeps failing keeps retrying.
