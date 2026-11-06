//! Request router: one short model call that decides which agent path (which guidance text) handles the request. It
//! sees the request's system prompt, the user's message and the live tool catalog (names and descriptions, each
//! description shortened to 220 characters), and no memory records; its tool names are advisory and unused.

