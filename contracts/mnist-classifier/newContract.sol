// SPDX-License-Identifier: GPL-3.0

pragma solidity >=0.7.0 <0.9.0;

contract NeuralNet {
    // Floating points
    struct Point {
        int16 top;
        int16 bot;
    }
    uint constant N = 28;
    uint constant M = 10;
    Point[M][N*N] public w;
    Point[N][N] public inp;

    function setWeight(uint row, uint column, int16 top, int16 bot) public {
        w[row][column] = NeuralNet.Point(top, bot);
    }

    function setTest(uint row, uint column, int16 top, int16 bot) public {
        inp[row][column] = NeuralNet.Point(top, bot);
    }

    function add(Point memory a, Point memory b) private pure returns(Point memory) {
        int16 top = a.top + b.top;
        int bota = a.bot; int botb = b.bot; require(bota == a.bot, "ok");
        int bot = bota + botb;
        if (bot >= 2**15) {
            top += 1;
            bot -= 2**15;
            require((0 <= bot) && (bot <= 2e15), "Shouldn't happen ?");
        }
        if (-bot >= 2**15) {
            top -= 1;
            bot += 2**15;
            require(-bot <= 2**15, "Shouldn't happen ?");
        }
        return Point({top: top, bot: int16(bot)});
    }
    function min(int a, int b) private pure returns(int) {
      if (a < b) return a;
      return b;
    }
    function multiply(Point memory a, Point memory b) private pure returns(Point memory) {
        int top_a = a.top; int bot_a = a.bot;
        int top_b = b.top; int bot_b = b.bot;
        int top; int bot;
        // top * top
        top += top_a * top_b;
        // top * bot
        bot += (top_a * bot_b);
        bot += (top_b * bot_a);
        // bot * bot
        bot += (bot_a * bot_b) >> 15;
        //Check for overflow
        require(top <= type(int16).max, "Overflow");
        top += bot / (2**15);
        bot = bot % (2**15);
        //Synchronize signs
        if (top != 0) {
          if (top < 0 && bot > 0) {
            bot -= 2**15;
            top += 1;
          }
          if (top > 0 && bot < 0) {
            bot += 2**15;
            top -= 1; 
          }
        }
        Point memory res;
        res.top = int16(top);
        res.bot = int16(bot);
        return res;
    }

    function setWeightLine(uint line, Point[N*N] calldata data) external {
        for (uint j = 0; j < N * N; j++)
            w[line][j] = data[j];
    }

    function classify() external view returns(Point[8] memory) {
        Point[M] memory b = [Point(0,-7573),Point(0,13283),Point(0,249),Point(0,-7315),Point(0,3027),Point(0,18609),Point(0,-2440),Point(0,9858),Point(0,-22136),Point(0,-3869)];
        Point[8] memory res = [Point(0,0),Point(0,0),Point(0,0),Point(0,0),Point(0,0),Point(0,0), Point(0,0), Point(0,0)];


        for (uint i = 0; i < N; i++) {
            for (uint j = 0; j < N; j++) {
                for (uint k = 0; k < 8; k++) {
                    res[k] = add(res[k], multiply(inp[i][j], w[i*28+j][k]));
                }
            }
        }
        for (uint k = 0; k < 8; k++) {
            res[k] = add(res[k], b[k]);
        }

        return res;
    }
}