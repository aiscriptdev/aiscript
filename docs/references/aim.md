
**aim** (AiScript Manager) is a command-line tool for AiScript. Similar [cargo](https://github.com/rust-lang/cargo) for Rust. It provides a set of commands to help you build web applications quickly.

## ai

`aim ai` is a command to generate AI apps built in AiScript from a prompt.

```
$ cat prompts.txt
Please write a chatbot apps that can answer questions about the
following topics:
- history
- geography

$ aim ai prompts.txt

```

## compile

## console

## deploy

Deploy the AiScript project to the aiscript.dev server.

```
$ aim deploy
```

## debug

Open interactive Postman like debug UI in the browser. **aim** will import the API routes into the debug UI.

```
$ aim debug
Opening http://localhost:8001
```

## fmt

Format the source code.

## gen

Generate code from templates.

## lint

Lint the source code.

## migrate

Database schema migration.

### migrate up

### migrate down

## mock

Generate mock data.

## new

Create a new AiScript project.

## test

## server

Run the AiScript project as a web server.

## version

Show the version of AiScript.
