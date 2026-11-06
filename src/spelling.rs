//! Spelling reading (declared advisory stage 0, policy W4 typo handling). One short model call, run concurrently
//! with the router, writes the user's message with its spelling mistakes corrected and nothing else changed. The
//! deciding stages receive it as a reading note next to the original message, which stays the request (W2). A
//! correction that changes any identifier or number, or the message's length by more than 30 %, is not used.

