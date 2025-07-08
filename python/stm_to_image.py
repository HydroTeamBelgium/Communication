import ast

with open("demo/myfile.txt", "r") as f:
    content = f.read()

# Turn the content of the text file into a list
u8_array = ast.literal_eval(content)  # now it's a real list of ints
# print(u8_array)

# Convert to bytes and save as JPG
byte_data = bytes(u8_array)
# print(byte_data)
with open("demo/restored_image.jpg", "wb") as f:
    f.write(byte_data)

print("Image successfully restored!")
