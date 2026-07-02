import express from "express";

const app = express();

app.get("/users", listUsers);
app.post("/users", createUser);
app.delete("/users/:id", removeUser);
