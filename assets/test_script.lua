local app = ...
function my_system(my_arg)
    print("hi world")
end

app:register_system(my_system)

print("hello world!")
