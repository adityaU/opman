export function isPermanentError(reason) {
    if (!reason)
        return false;
    return (reason.includes('Invalid refresh token') ||
        reason.includes('Invalid grant provided') ||
        reason.includes('invalid_grant') ||
        reason.includes('ExpiredTokenException') ||
        reason.includes('InvalidTokenException') ||
        reason.includes('HTTP_401') ||
        reason.includes('HTTP_403'));
}
