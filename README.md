# KVStore

A super simple Rust + Axum + Redis implementation of a key-value storage over HTTP API

## Authentication

Each request requires a Bearer-token set and to be found from Redis in table called `tokens`

## Endpoints

### `GET /key`

Returns the given value as a JSON string

### `POST /key`
Body: 
```
{
	"value": "this is a value"
}
```

Sets the value

### `DELETE /key`

Deletes a value