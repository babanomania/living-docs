const { UserService } = require("./user-service");

function bootstrap() {
  return new UserService();
}

module.exports = { bootstrap };
