import os

print(os.getcwd())

with open("utils/good.log") as good:
    with open("utils/bad.log") as file:
        while line := file.readline().rstrip().lower():
            line2 = good.readline().rstrip().lower()
            ok = "  " if line2.startswith(line) else "--"
            print(ok + " " + line + " " + line2)
            # if not line2.startswith(line):
            #     print("err")
            #     exit(1)
